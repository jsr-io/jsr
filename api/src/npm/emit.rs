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

use crate::npm::import_transform::ImportRewriteTransformer;

use super::specifiers::RewriteKind;
use super::specifiers::SpecifierRewriter;

pub fn transpile_to_js(
  source: &ParsedSource,
  specifier_rewriter: SpecifierRewriter,
) -> Result<String, anyhow::Error> {
  let emit_options = deno_ast::EmitOptions {
    source_map: SourceMapOption::Inline,
    source_map_file: None,
    inline_sources: false,
    keep_comments: true,
  };

  let file_name = source.specifier().path().split('/').last().unwrap();
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

    let emitted = emit(&program, &comments, &source_map, &emit_options)?;

    Ok(emitted.text)
  })
}

pub fn transpile_to_dts(
  source: &ParsedSource,
  fast_check_module: &FastCheckTypeModule,
  specifier_rewriter: SpecifierRewriter,
) -> Result<String, anyhow::Error> {
  let dts = fast_check_module.dts.as_ref().unwrap();

  let emit_options = deno_ast::EmitOptions {
    source_map: SourceMapOption::Inline,
    source_map_file: None,
    inline_sources: false,
    keep_comments: true,
  };

  let file_name = source.specifier().path().split('/').last().unwrap();
  let source_map =
    SourceMap::single(file_name, source.text_info().text_str().to_owned());

  let comments = dts.comments.as_single_threaded();

  let mut program = dts.program.clone();

  let mut import_rewrite_transformer = ImportRewriteTransformer {
    specifier_rewriter,
    kind: RewriteKind::Declaration,
  };
  program.visit_mut_with(&mut import_rewrite_transformer);

  let EmittedSource { text, .. } =
    emit(&program, &comments, &source_map, &emit_options)?;

  Ok(text)
}
