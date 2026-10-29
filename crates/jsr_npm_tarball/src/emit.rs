// Copyright 2024 the JSR authors. All rights reserved. MIT license.

use deno_ast::DecoratorsTranspileOption;
use deno_ast::EmittedSourceText;
use deno_ast::JsxAutomaticOptions;
use deno_ast::JsxClassicOptions;
use deno_ast::JsxPrecompileOptions;
use deno_ast::JsxRuntime;
use deno_ast::ParsedSource;
use deno_ast::SourceMap;
use deno_ast::SourceMapOption;
use deno_ast::TranspileOptions;
use deno_ast::emit;
use deno_ast::fold_program;
use deno_ast::swc::ecma_visit::VisitMutWith;
use deno_graph::FastCheckTypeModule;
use url::Url;

use crate::import_transform::ImportRewriteTransformer;
use crate::specifiers::relative_import_specifier;

use super::specifiers::RewriteKind;
use super::specifiers::SpecifierRewriter;

/// Transpiles a module to JavaScript, returning the code and, if
/// `source_maps` is set, its source map.
pub fn transpile_to_js(
  source: &ParsedSource,
  specifier_rewriter: SpecifierRewriter,
  target_specifier: &Url,
  transpile_options: &TranspileOptions,
  source_maps: bool,
) -> Result<(Vec<u8>, Option<Vec<u8>>), anyhow::Error> {
  let basename = target_specifier.path().rsplit_once('/').unwrap().1;
  let emit_options = deno_ast::EmitOptions {
    source_map: source_map_option(source_maps),
    source_map_file: Some(basename.to_owned()),
    source_map_base: None,
    inline_sources: false,
    remove_comments: false,
  };

  let file_name =
    relative_import_specifier(target_specifier, source.specifier());
  let source_map = SourceMap::single(file_name, source.text().to_string());

  let mut program = source.program_ref().to_owned();

  // needs to align with what's done internally in source map
  assert_eq!(1, source.range().start.as_byte_pos().0);
  // we need the comments to be mutable, so make it single threaded
  let comments = source.comments().as_single_threaded();
  source.globals().with(|marks| {
    let mut import_rewrite_transformer = ImportRewriteTransformer {
      specifier_rewriter,
      kind: RewriteKind::Source,
    };
    program.visit_mut_with(&mut import_rewrite_transformer);

    let program = fold_program(
      program,
      transpile_options,
      &source_map,
      &comments,
      marks,
      Box::new(source.diagnostics().iter()),
    )?;

    let emitted =
      emit((&program).into(), &comments, &source_map, &emit_options)?;
    Ok(finish_emit(emitted, basename))
  })
}

/// Emits the declarations of a module, returning the code and, if
/// `source_maps` is set, its source map.
pub fn transpile_to_dts(
  source: &ParsedSource,
  fast_check_module: &FastCheckTypeModule,
  specifier_rewriter: SpecifierRewriter,
  target_specifier: &Url,
  source_maps: bool,
) -> Result<(Vec<u8>, Option<Vec<u8>>), anyhow::Error> {
  let dts = fast_check_module.dts.as_ref().unwrap();

  let basename = target_specifier.path().rsplit_once('/').unwrap().1;
  let emit_options = deno_ast::EmitOptions {
    source_map: source_map_option(source_maps),
    source_map_file: Some(basename.to_owned()),
    source_map_base: None,
    inline_sources: false,
    remove_comments: false,
  };

  let file_name =
    relative_import_specifier(target_specifier, source.specifier());
  let source_map = SourceMap::single(file_name, source.text().to_string());

  let comments = dts.comments.as_single_threaded();

  let mut program = dts.program.clone();

  let mut import_rewrite_transformer = ImportRewriteTransformer {
    specifier_rewriter,
    kind: RewriteKind::Declaration,
  };
  program.visit_mut_with(&mut import_rewrite_transformer);

  let emitted = emit((&program).into(), &comments, &source_map, &emit_options)?;
  Ok(finish_emit(emitted, basename))
}

fn source_map_option(source_maps: bool) -> SourceMapOption {
  if source_maps {
    SourceMapOption::Separate
  } else {
    SourceMapOption::None
  }
}

fn finish_emit(
  emitted: EmittedSourceText,
  basename: &str,
) -> (Vec<u8>, Option<Vec<u8>>) {
  let EmittedSourceText { text, source_map } = emitted;
  let mut source = text.into_bytes();

  if let Some(last) = source.last()
    && *last != b'\n'
  {
    source.push(b'\n');
  }

  if source_map.is_some() {
    source
      .extend(format!("//# sourceMappingURL={}.map", basename).into_bytes());
  }

  (source, source_map.map(String::into_bytes))
}

/// Creates the transpile options for a package from the `compilerOptions` of
/// its config file. Without them, JSX uses the classic runtime with
/// `React.createElement`, and decorators are ECMAScript decorators.
pub fn transpile_options_from_compiler_options(
  compiler_options: Option<&serde_json::Value>,
) -> TranspileOptions {
  let get_str = |key: &str| -> Option<String> {
    compiler_options?.get(key)?.as_str().map(|s| s.to_string())
  };
  let get_bool = |key: &str| -> bool {
    compiler_options
      .and_then(|options| options.get(key))
      .and_then(|value| value.as_bool())
      .unwrap_or(false)
  };

  let jsx_import_source = get_str("jsxImportSource");
  let jsx = match get_str("jsx").as_deref() {
    Some("react-jsx") => Some(JsxRuntime::Automatic(JsxAutomaticOptions {
      development: false,
      import_source: jsx_import_source,
    })),
    Some("react-jsxdev") => Some(JsxRuntime::Automatic(JsxAutomaticOptions {
      development: true,
      import_source: jsx_import_source,
    })),
    Some("precompile") => Some(JsxRuntime::Precompile(JsxPrecompileOptions {
      automatic: JsxAutomaticOptions {
        development: false,
        import_source: jsx_import_source,
      },
      skip_elements: None,
      dynamic_props: None,
    })),
    _ => {
      let default = JsxClassicOptions::default();
      Some(JsxRuntime::Classic(JsxClassicOptions {
        factory: get_str("jsxFactory").unwrap_or(default.factory),
        fragment_factory: get_str("jsxFragmentFactory")
          .unwrap_or(default.fragment_factory),
      }))
    }
  };

  let decorators = if get_bool("experimentalDecorators") {
    DecoratorsTranspileOption::LegacyTypeScript {
      emit_metadata: get_bool("emitDecoratorMetadata"),
    }
  } else {
    DecoratorsTranspileOption::Ecma
  };

  TranspileOptions {
    jsx,
    decorators,
    ..Default::default()
  }
}
