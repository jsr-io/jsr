// Copyright 2024 the JSR authors. All rights reserved. MIT license.

use deno_ast::ParsedSource;
use deno_ast::SourceMap;
use deno_ast::SourceMapOption;
use deno_ast::TranspileOptions;
use deno_ast::emit;
use deno_ast::fold_program;
use deno_ast::swc::ecma_visit::VisitMutWith;
use deno_ast::{DecoratorsTranspileOption, EmittedSourceText};
use deno_ast::{JsxAutomaticOptions, JsxClassicOptions, JsxRuntime};
use deno_graph::FastCheckTypeModule;
use url::Url;

use crate::npm::import_transform::ImportRewriteTransformer;
use crate::npm::specifiers::relative_import_specifier;

use super::specifiers::RewriteKind;
use super::specifiers::SpecifierRewriter;

/// JSX settings taken from the package compilerOptions and file pragmas.
#[derive(Debug, Clone, Default)]
pub struct JsxEmitOptions {
  pub jsx: Option<String>,
  pub jsx_import_source: Option<String>,
  pub jsx_factory: Option<String>,
  pub jsx_fragment_factory: Option<String>,
}

pub fn transpile_to_js(
  source: &ParsedSource,
  specifier_rewriter: SpecifierRewriter,
  target_specifier: &Url,
  jsx_options: &JsxEmitOptions,
) -> Result<(Vec<u8>, Vec<u8>), anyhow::Error> {
  let basename = target_specifier.path().rsplit_once('/').unwrap().1;
  let emit_options = deno_ast::EmitOptions {
    source_map: SourceMapOption::Separate,
    source_map_file: Some(basename.to_owned()),
    source_map_base: None,
    inline_sources: false,
    remove_comments: false,
  };

  let file_name =
    relative_import_specifier(target_specifier, source.specifier());
  let source_map = SourceMap::single(file_name, source.text().to_string());

  let program = source.program_ref().to_owned();

  // needs to align with what's done internally in source map
  assert_eq!(1, source.range().start.as_byte_pos().0);
  // we need the comments to be mutable, so make it single threaded
  let comments = source.comments().as_single_threaded();
  source.globals().with(|marks| {
    let transpile_options = TranspileOptions {
      decorators: DecoratorsTranspileOption::Ecma,
      jsx: resolve_jsx_runtime(source, jsx_options),
      ..Default::default()
    };

    let mut program = fold_program(
      program,
      &transpile_options,
      &source_map,
      &comments,
      marks,
      Box::new(source.diagnostics().iter()),
    )?;

    let mut import_rewrite_transformer = ImportRewriteTransformer {
      specifier_rewriter,
      kind: RewriteKind::Source,
    };
    program.visit_mut_with(&mut import_rewrite_transformer);

    let EmittedSourceText { text, source_map } =
      emit((&program).into(), &comments, &source_map, &emit_options)?;
    let mut source = text.into_bytes();

    if let Some(last) = source.last()
      && *last != b'\n'
    {
      source.push(b'\n');
    }

    source
      .extend(format!("//# sourceMappingURL={}.map", basename).into_bytes());

    Ok((source, source_map.unwrap().into_bytes()))
  })
}

pub fn transpile_to_dts(
  source: &ParsedSource,
  fast_check_module: &FastCheckTypeModule,
  specifier_rewriter: SpecifierRewriter,
  target_specifier: &Url,
) -> Result<(Vec<u8>, Vec<u8>), anyhow::Error> {
  let dts = fast_check_module.dts.as_ref().unwrap();

  let basename = target_specifier.path().rsplit_once('/').unwrap().1;
  let emit_options = deno_ast::EmitOptions {
    source_map: SourceMapOption::Separate,
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

  let EmittedSourceText { text, source_map } =
    emit((&program).into(), &comments, &source_map, &emit_options)?;
  let mut source = text.into_bytes();

  if let Some(last) = source.last()
    && *last != b'\n'
  {
    source.push(b'\n');
  }

  source.extend(format!("//# sourceMappingURL={}.map", basename).into_bytes());

  Ok((source, source_map.unwrap().into_bytes()))
}

fn resolve_jsx_runtime(
  source: &ParsedSource,
  jsx_options: &JsxEmitOptions,
) -> Option<JsxRuntime> {
  if !matches!(
    source.media_type(),
    deno_ast::MediaType::Jsx | deno_ast::MediaType::Tsx
  ) {
    return None;
  }

  let pragma_import_source = jsx_import_source_from_pragma(source.text());
  let import_source = pragma_import_source
    .clone()
    .or_else(|| jsx_options.jsx_import_source.clone());
  let jsx = jsx_options.jsx.as_deref();

  let automatic = JsxAutomaticOptions {
    development: matches!(jsx, Some("react-jsxdev")),
    import_source,
  };

  match jsx {
    Some("precompile") => {
      Some(JsxRuntime::Precompile(deno_ast::JsxPrecompileOptions {
        automatic,
        skip_elements: None,
        dynamic_props: None,
      }))
    }
    Some("react-jsx") | Some("react-jsxdev") => {
      Some(JsxRuntime::Automatic(automatic))
    }
    _ => {
      if pragma_import_source.is_some()
        || jsx_options.jsx_import_source.is_some()
      {
        Some(JsxRuntime::Automatic(automatic))
      } else {
        Some(classic_jsx_runtime(jsx_options))
      }
    }
  }
}

fn classic_jsx_runtime(jsx_options: &JsxEmitOptions) -> JsxRuntime {
  JsxRuntime::Classic(JsxClassicOptions {
    factory: jsx_options
      .jsx_factory
      .clone()
      .unwrap_or_else(|| "React.createElement".into()),
    fragment_factory: jsx_options
      .jsx_fragment_factory
      .clone()
      .unwrap_or_else(|| "React.Fragment".into()),
  })
}

fn jsx_import_source_from_pragma(text: &str) -> Option<String> {
  for line in text.lines().take(50) {
    let Some(idx) = line.find("@jsxImportSource") else {
      continue;
    };
    let rest = line[idx + "@jsxImportSource".len()..].trim();
    let value = rest
      .split(|c: char| c.is_whitespace() || c == '*')
      .find(|s| !s.is_empty() && *s != "*/")?;
    let value = value.trim_end_matches("*/").trim();
    if !value.is_empty() {
      return Some(value.to_string());
    }
  }
  None
}
