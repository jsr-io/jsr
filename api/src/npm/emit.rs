// Copyright 2024 the JSR authors. All rights reserved. MIT license.

use deno_ast::emit;
use deno_ast::fold_program;
use deno_ast::swc::ecma_visit::VisitMutWith;
use deno_ast::ParsedSource;
use deno_ast::SourceMap;
use deno_ast::SourceMapOption;
use deno_ast::TranspileOptions;
use deno_ast::{DecoratorsTranspileOption, EmittedSourceText};
use deno_ast::{JsxAutomaticOptions, JsxClassicOptions, JsxRuntime};
use deno_graph::FastCheckTypeModule;
use url::Url;

use crate::npm::import_transform::ImportRewriteTransformer;
use crate::npm::specifiers::relative_import_specifier;

use super::specifiers::RewriteKind;
use super::specifiers::SpecifierRewriter;

pub fn transpile_to_js(
  source: &ParsedSource,
  specifier_rewriter: SpecifierRewriter,
  target_specifier: &Url,
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
      jsx: resolve_jsx_runtime(source),
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

fn resolve_jsx_runtime(source: &ParsedSource) -> Option<JsxRuntime> {
  if !matches!(
    source.media_type(),
    deno_ast::MediaType::Jsx | deno_ast::MediaType::Tsx
  ) {
    return None;
  }

  let header: String = source
    .text()
    .lines()
    .take(50)
    .collect::<Vec<_>>()
    .join("\n");
  let import_source = pragma_value(&header, "@jsxImportSource");
  let runtime = pragma_value(&header, "@jsxRuntime");
  let factory = pragma_value(&header, "@jsxFactory");
  let fragment_factory = pragma_value(&header, "@jsxFragmentFactory");

  let automatic = JsxAutomaticOptions {
    development: matches!(runtime.as_deref(), Some("react-jsxdev")),
    import_source: import_source.clone(),
  };

  match runtime.as_deref() {
    Some("precompile") => {
      Some(JsxRuntime::Precompile(deno_ast::JsxPrecompileOptions {
        automatic,
        skip_elements: None,
        dynamic_props: None,
      }))
    }
    Some("automatic") | Some("react-jsx") | Some("react-jsxdev") => {
      Some(JsxRuntime::Automatic(automatic))
    }
    Some("classic") => Some(classic_jsx_runtime(factory, fragment_factory)),
    _ => {
      if import_source.is_some() {
        Some(JsxRuntime::Automatic(automatic))
      } else {
        Some(classic_jsx_runtime(factory, fragment_factory))
      }
    }
  }
}

fn classic_jsx_runtime(
  factory: Option<String>,
  fragment_factory: Option<String>,
) -> JsxRuntime {
  JsxRuntime::Classic(JsxClassicOptions {
    factory: factory.unwrap_or_else(|| "React.createElement".into()),
    fragment_factory: fragment_factory
      .unwrap_or_else(|| "React.Fragment".into()),
  })
}

/// Value after `@tag`. `@jsxImportSource` must not match `@jsxImportSourceTypes`.
fn pragma_value(text: &str, tag: &str) -> Option<String> {
  let mut search_from = 0;
  while let Some(rel) = text[search_from..].find(tag) {
    let idx = search_from + rel;
    let after = &text[idx + tag.len()..];
    if after.starts_with(|c: char| c.is_ascii_alphanumeric()) {
      search_from = idx + tag.len();
      continue;
    }
    let rest = after.trim_start();
    let value = rest
      .split(|c: char| c.is_whitespace() || c == '*')
      .find(|s| !s.is_empty() && *s != "*/")?;
    let value = value.trim_end_matches("*/").trim();
    if !value.is_empty() {
      return Some(value.to_string());
    }
    search_from = idx + tag.len();
  }
  None
}

#[cfg(test)]
mod tests {
  use super::pragma_value;

  #[test]
  fn pragma_value_skips_import_source_types() {
    let text = concat!(
      "/** @jsxRuntime automatic */",
      "/** @jsxImportSource preact@^10.29.2 */",
      "/** @jsxImportSourceTypes preact@^10.29.2 */",
    );
    assert_eq!(
      pragma_value(text, "@jsxImportSource").as_deref(),
      Some("preact@^10.29.2"),
    );
    assert_eq!(
      pragma_value(text, "@jsxImportSourceTypes").as_deref(),
      Some("preact@^10.29.2"),
    );
    assert_eq!(
      pragma_value(text, "@jsxRuntime").as_deref(),
      Some("automatic"),
    );
  }

  #[test]
  fn pragma_value_does_not_match_types_as_import_source() {
    let text = concat!(
      "/** @jsxImportSourceTypes preact@^10.29.2 */",
      "/** @jsxImportSource preact@^10.29.2 */",
    );
    assert_eq!(
      pragma_value(text, "@jsxImportSource").as_deref(),
      Some("preact@^10.29.2"),
    );
  }

  #[test]
  fn pragma_value_classic_factories() {
    let text = concat!(
      "/** @jsxRuntime classic */",
      "/** @jsxFactory h */",
      "/** @jsxFragmentFactory Fragment */",
    );
    assert_eq!(
      pragma_value(text, "@jsxRuntime").as_deref(),
      Some("classic"),
    );
    assert_eq!(pragma_value(text, "@jsxFactory").as_deref(), Some("h"));
    assert_eq!(
      pragma_value(text, "@jsxFragmentFactory").as_deref(),
      Some("Fragment"),
    );
  }
}
