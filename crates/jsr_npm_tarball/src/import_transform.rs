// Copyright 2024 the JSR authors. All rights reserved. MIT license.
use deno_ast::SourceRange;
use deno_ast::SourceRangedForSpanned;
use deno_ast::swc::ast::CallExpr;
use deno_ast::swc::ast::Callee;
use deno_ast::swc::ast::ExportAll;
use deno_ast::swc::ast::Expr;
use deno_ast::swc::ast::ExprOrSpread;
use deno_ast::swc::ast::ImportDecl;
use deno_ast::swc::ast::Lit;
use deno_ast::swc::ast::MemberProp;
use deno_ast::swc::ast::MetaPropKind;
use deno_ast::swc::ast::NamedExport;
use deno_ast::swc::ast::Str;
use deno_ast::swc::ast::TsImportType;
use deno_ast::swc::ecma_visit::Visit;
use deno_ast::swc::ecma_visit::VisitMut;
use deno_ast::swc::ecma_visit::VisitMutWith;
use deno_ast::swc::ecma_visit::VisitWith;

use super::specifiers::RewriteKind;
use super::specifiers::SpecifierRewriter;

pub struct ImportRewriteTransformer<'a> {
  pub specifier_rewriter: SpecifierRewriter<'a>,
  pub kind: RewriteKind,
}

impl VisitMut for ImportRewriteTransformer<'_> {
  fn visit_mut_import_decl(&mut self, node: &mut ImportDecl) {
    node.visit_mut_children_with(self);

    if let Some(value) = node.src.value.as_str()
      && let Some(remapped) = self.specifier_rewriter.rewrite(value, self.kind)
    {
      node.src = Box::new(remapped.into());
    }
  }

  fn visit_mut_named_export(&mut self, node: &mut NamedExport) {
    node.visit_mut_children_with(self);

    if let Some(src) = &node.src
      && let Some(value) = src.value.as_str()
      && let Some(remapped) = self.specifier_rewriter.rewrite(value, self.kind)
    {
      node.src = Some(Box::new(remapped.into()));
    }
  }

  fn visit_mut_export_all(&mut self, node: &mut ExportAll) {
    node.visit_mut_children_with(self);

    if let Some(value) = node.src.value.as_str()
      && let Some(remapped) = self.specifier_rewriter.rewrite(value, self.kind)
    {
      node.src = Box::new(remapped.into());
    }
  }

  fn visit_mut_ts_import_type(&mut self, n: &mut TsImportType) {
    n.visit_mut_children_with(self);

    if let Some(value) = n.arg.value.as_str()
      && let Some(remapped) = self
        .specifier_rewriter
        .rewrite(value, RewriteKind::Declaration)
    {
      n.arg = remapped.into();
    }
  }

  fn visit_mut_call_expr(&mut self, node: &mut CallExpr) {
    node.visit_mut_children_with(self);

    let is_import = matches!(node.callee, Callee::Import(_));
    if (is_import || import_meta_resolve_arg(node).is_some())
      && let Some(arg) = node.args.first()
      && let Expr::Lit(Lit::Str(lit_str)) = *arg.expr.clone()
      && let Some(value) = lit_str.value.as_str()
    {
      let maybe_rewritten = if is_import {
        self.specifier_rewriter.rewrite(value, self.kind)
      } else {
        self.specifier_rewriter.rewrite_unanalyzed(value)
      };
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

/// Returns the string argument of an `import.meta.resolve("...")` call.
fn import_meta_resolve_arg(node: &CallExpr) -> Option<&Str> {
  if node.args.len() == 1
    && let Some(arg) = node.args.first()
    && arg.spread.is_none()
    && let Callee::Expr(callee) = &node.callee
    && let Expr::Member(member) = &**callee
    && let Expr::MetaProp(prop) = &*member.obj
    && prop.kind == MetaPropKind::ImportMeta
    && let MemberProp::Ident(ident) = &member.prop
    && ident.sym == "resolve"
    && let Expr::Lit(Lit::Str(lit_str)) = &*arg.expr
  {
    Some(lit_str)
  } else {
    None
  }
}

/// Collects the string arguments of `import.meta.resolve()` calls, which
/// deno_graph doesn't analyze as dependencies.
#[derive(Default)]
pub struct ImportMetaResolveCollector {
  /// The range of the string literal, including its quotes, and its value.
  pub specifiers: Vec<(SourceRange, String)>,
}

impl Visit for ImportMetaResolveCollector {
  fn visit_call_expr(&mut self, node: &CallExpr) {
    node.visit_children_with(self);

    if let Some(lit_str) = import_meta_resolve_arg(node)
      && let Some(value) = lit_str.value.as_str()
    {
      self.specifiers.push((lit_str.range(), value.to_string()));
    }
  }
}
