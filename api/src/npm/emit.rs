// Copyright 2024 the JSR authors. All rights reserved. MIT license.

use deno_ast::emit;
use deno_ast::fold_program;
use deno_ast::swc::ast::CallExpr;
use deno_ast::swc::ast::Callee;
use deno_ast::swc::ast::ExportAll;
use deno_ast::swc::ast::Expr;
use deno_ast::swc::ast::ExprOrSpread;
use deno_ast::swc::ast::ImportDecl;
use deno_ast::swc::ast::Lit;
use deno_ast::swc::ast::Module;
use deno_ast::swc::ast::NamedExport;
use deno_ast::swc::ast::Str;
use deno_ast::swc::common::Globals;
use deno_ast::swc::common::Mark;
use deno_ast::swc::visit::as_folder;
use deno_ast::swc::visit::noop_visit_mut_type;
use deno_ast::swc::visit::FoldWith;
use deno_ast::swc::visit::VisitMut;
use deno_ast::swc::visit::VisitMutWith;
use deno_ast::ParsedSource;
use deno_ast::SourceMap;
use deno_ast::SourceMapOption;
use url::Url;

use super::specifiers::rewrite_specifier;

// todo: a lot of code is duplicated with `ParsedSource::transpile` because we
// can't fold a `ParsedSource` directly.
pub fn transpile_to_js(
  source: ParsedSource,
  source_url: Url,
) -> Result<String, anyhow::Error> {
  let transpile_options = deno_ast::TranspileOptions {
    // FIXME: JSX?
    ..Default::default()
  };
  let emit_options = deno_ast::EmitOptions {
    source_map: SourceMapOption::Inline,
    inline_sources: true,
    keep_comments: true,
  };

  let source_map =
    SourceMap::single(source_url, source.text_info().text_str().to_string());

  let mut folder = as_folder(NpmImportTransform);
  let program = source.program_ref().clone().fold_with(&mut folder);

  // needs to align with what's done internally in source map
  assert_eq!(1, source.text_info().range().start.as_byte_pos().0);
  // we need the comments to be mutable, so make it single threaded
  let comments = source.comments().as_single_threaded();
  let globals = Globals::new();
  deno_ast::swc::common::GLOBALS.set(&globals, || {
    let top_level_mark = Mark::fresh(Mark::root());
    let program = fold_program(
      program,
      &transpile_options,
      &source_map,
      &comments,
      top_level_mark,
      source.diagnostics(),
    )?;

    let emitted = emit(&program, &comments, &source_map, &emit_options)?;

    Ok(emitted.text)
  })
}

pub struct NpmImportTransform;

impl VisitMut for NpmImportTransform {
  noop_visit_mut_type!();

  fn visit_mut_module(&mut self, module: &mut Module) {
    module.visit_mut_children_with(self);
  }

  fn visit_mut_import_decl(&mut self, node: &mut ImportDecl) {
    node.visit_mut_children_with(self);

    if let Some(remapped) = rewrite_specifier(&node.src.value) {
      node.src = Box::new(remapped.into());
    }
  }

  fn visit_mut_named_export(&mut self, node: &mut NamedExport) {
    node.visit_mut_children_with(self);

    if let Some(src) = &node.src {
      if let Some(remapped) = rewrite_specifier(&src.value) {
        node.src = Some(Box::new(remapped.into()));
      }
    }
  }

  fn visit_mut_export_all(&mut self, node: &mut ExportAll) {
    node.visit_mut_children_with(self);

    if let Some(remapped) = rewrite_specifier(&node.src.value) {
      node.src = Box::new(remapped.into());
    }
  }

  fn visit_mut_call_expr(&mut self, node: &mut CallExpr) {
    node.visit_mut_children_with(self);

    if let Callee::Import(_) = node.callee {
      if let Some(arg) = node.args.first() {
        if let Expr::Lit(Lit::Str(lit_str)) = *arg.expr.clone() {
          let maybe_rewritten = rewrite_specifier(&lit_str.value);
          if let Some(rewritten) = maybe_rewritten {
            let replacer = Expr::Lit(Lit::Str(Str {
              span: lit_str.span,
              value: rewritten.into(),
              raw: None,
            }));
            node.args[0] = ExprOrSpread {
              spread: None,
              expr: Box::new(replacer),
            };
          }
        }
      }
    }
  }
}

#[cfg(test)]
mod tests {
  use deno_ast::ModuleSpecifier;
  use deno_ast::ParseParams;
  use deno_ast::SourceTextInfo;

  use super::transpile_to_js;

  fn test_transform(source: &str, expect: &str) {
    let specifier = "file:///main.ts";

    let parsed_source = deno_ast::parse_module(ParseParams {
      specifier: ModuleSpecifier::parse(specifier).unwrap(),
      media_type: deno_ast::MediaType::TypeScript,
      text_info: SourceTextInfo::new(source.into()),
      capture_tokens: false,
      scope_analysis: false,
      maybe_syntax: None,
    })
    .unwrap();

    let result =
      transpile_to_js(parsed_source, specifier.parse().unwrap()).unwrap();
    let (result, _) = result
      .rsplit_once("\n//# sourceMappingURL=data:application/json;base64,")
      .unwrap();

    assert_eq!(result.trim_end(), expect);
  }

  #[test]
  fn test_transform_specifiers() {
    test_transform(r#"import "./foo/bar.ts";"#, r#"import "./foo/bar.js";"#);
    test_transform(r#"import "../foo.tsx";"#, r#"import "../foo.js";"#);
    test_transform(r#"import "jsr:@std/path";"#, r#"import "@jsr/std__path";"#);
    test_transform(r#"import "npm:@std/path";"#, r#"import "@std/path";"#);

    test_transform(
      "import * as foo from \"./bar.ts\";\nexport { foo };",
      "import * as foo from \"./bar.js\";\nexport { foo };",
    );
    test_transform(
      "import { asd } from \"./bar.ts\";\nexport { asd };",
      "import { asd } from \"./bar.js\";\nexport { asd };",
    );
    test_transform(
      "import { asd as foo } from \"./bar.ts\";\nexport { foo };",
      "import { asd as foo } from \"./bar.js\";\nexport { foo };",
    );
    test_transform(
      "import { asd, foo } from \"./bar.ts\";\nexport { asd, foo };",
      "import { asd, foo } from \"./bar.js\";\nexport { asd, foo };",
    );
    test_transform(
      "import asd from \"./bar.ts\";\nexport { asd };",
      "import asd from \"./bar.js\";\nexport { asd };",
    );
    test_transform(
      "import asd, { foo } from \"./bar.ts\";\nexport { asd, foo };",
      "import asd, { foo } from \"./bar.js\";\nexport { asd, foo };",
    );

    test_transform(
      "export * from \"./foo/bar.ts\";",
      "export * from \"./foo/bar.js\";",
    );
    test_transform(
      "export * as foo from \"./foo/bar.ts\";",
      "export * as foo from \"./foo/bar.js\";",
    );
    test_transform(
      "export { asd } from \"./foo/bar.ts\";",
      "export { asd } from \"./foo/bar.js\";",
    );
    test_transform(
      "export { asd as foo } from \"./foo/bar.ts\";",
      "export { asd as foo } from \"./foo/bar.js\";",
    );

    test_transform(
      "await import(\"./foo/bar.ts\");",
      "await import(\"./foo/bar.js\");",
    );
  }
}
