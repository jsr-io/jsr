// Copyright 2024 the JSR authors. All rights reserved. MIT license.

use deno_ast::emit;
use deno_ast::fold_program;
use deno_ast::swc::visit::VisitMutWith;
use deno_ast::EmittedSource;
use deno_ast::ParsedSource;
use deno_ast::SourceMap;
use deno_ast::SourceMapOption;
use deno_ast::TranspileOptions;
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
) -> Result<(String, String), anyhow::Error> {
  let basename = target_specifier.path().rsplit_once('/').unwrap().1;
  let emit_options = deno_ast::EmitOptions {
    source_map: SourceMapOption::Separate,
    source_map_file: Some(basename.to_owned()),
    inline_sources: false,
    keep_comments: true,
  };

  let file_name =
    relative_import_specifier(target_specifier, source.specifier());
  let source_map =
    SourceMap::single(file_name, source.text_info().text_str().to_owned());

  let mut program = source.program_ref().clone();

  // needs to align with what's done internally in source map
  assert_eq!(1, source.text_info().range().start.as_byte_pos().0);
  // we need the comments to be mutable, so make it single threaded
  let comments = source.comments().as_single_threaded();
  source.globals().with(|marks| {
    let mut import_rewrite_transformer = ImportRewriteTransformer {
      specifier_rewriter,
      kind: RewriteKind::Source,
    };
    program.visit_mut_with(&mut import_rewrite_transformer);

    let transpile_options = TranspileOptions {
      use_decorators_proposal: true,
      use_ts_decorators: false,

      // TODO: JSX
      ..Default::default()
    };

    let program = fold_program(
      program,
      &transpile_options,
      &source_map,
      &comments,
      marks,
      source.diagnostics(),
    )?;

    let EmittedSource {
      mut text,
      source_map,
    } = emit(&program, &comments, &source_map, &emit_options)?;
    if !text.ends_with('\n') {
      text.push('\n');
    }
    text.push_str(format!("//# sourceMappingURL={}.map", basename).as_str());

    Ok((text, source_map.unwrap()))
  })
}

pub fn transpile_to_dts(
  source: &ParsedSource,
  fast_check_module: &FastCheckTypeModule,
  specifier_rewriter: SpecifierRewriter,
  target_specifier: &Url,
) -> Result<(String, String), anyhow::Error> {
  let dts = fast_check_module.dts.as_ref().unwrap();

  let basename = target_specifier.path().rsplit_once('/').unwrap().1;
  let emit_options = deno_ast::EmitOptions {
    source_map: SourceMapOption::Separate,
    source_map_file: Some(basename.to_owned()),
    inline_sources: false,
    keep_comments: true,
  };

  let file_name =
    relative_import_specifier(target_specifier, source.specifier());
  let source_map =
    SourceMap::single(file_name, source.text_info().text_str().to_owned());

  let comments = dts.comments.as_single_threaded();

  let mut program = dts.program.clone();

  let mut import_rewrite_transformer = ImportRewriteTransformer {
    specifier_rewriter,
    kind: RewriteKind::Declaration,
  };
  program.visit_mut_with(&mut import_rewrite_transformer);

  let EmittedSource {
    mut text,
    source_map,
  } = emit(&program, &comments, &source_map, &emit_options)?;
  if !text.ends_with('\n') {
    text.push('\n');
  }
  text.push_str(format!("//# sourceMappingURL={}.map", basename).as_str());

  Ok((text, source_map.unwrap()))
}
