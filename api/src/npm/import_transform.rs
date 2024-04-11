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
use deno_ast::swc::ast::TsImportType;
use deno_ast::swc::visit::VisitMut;
use deno_ast::swc::visit::VisitMutWith;

use super::specifiers::RewriteKind;
use super::specifiers::SpecifierRewriter;

pub struct ImportRewriteTransformer<'a> {
  pub specifier_rewriter: SpecifierRewriter<'a>,
  pub kind: RewriteKind,
}

impl<'a> VisitMut for ImportRewriteTransformer<'a> {
  fn visit_mut_module(&mut self, module: &mut Module) {
    module.visit_mut_children_with(self);
  }

  fn visit_mut_import_decl(&mut self, node: &mut ImportDecl) {
    node.visit_mut_children_with(self);

    if let Some(remapped) = self
      .specifier_rewriter
      .rewrite(&node.src.value.as_str(), self.kind)
    {
      node.src = Box::new(remapped.into());
    }
  }

  fn visit_mut_named_export(&mut self, node: &mut NamedExport) {
    node.visit_mut_children_with(self);

    if let Some(src) = &node.src {
      if let Some(remapped) = self
        .specifier_rewriter
        .rewrite(&src.value.as_str(), self.kind)
      {
        node.src = Some(Box::new(remapped.into()));
      }
    }
  }

  fn visit_mut_export_all(&mut self, node: &mut ExportAll) {
    node.visit_mut_children_with(self);

    if let Some(remapped) = self
      .specifier_rewriter
      .rewrite(&node.src.value.as_str(), self.kind)
    {
      node.src = Box::new(remapped.into());
    }
  }

  fn visit_mut_ts_import_type(&mut self, n: &mut TsImportType) {
    n.visit_mut_children_with(self);

    if let Some(remapped) = self
      .specifier_rewriter
      .rewrite(&n.arg.value.as_str(), RewriteKind::Declaration)
    {
      n.arg = remapped.into();
    }
  }

  fn visit_mut_call_expr(&mut self, node: &mut CallExpr) {
    node.visit_mut_children_with(self);

    if let Callee::Import(_) = node.callee {
      if let Some(arg) = node.args.first() {
        if let Expr::Lit(Lit::Str(lit_str)) = *arg.expr.clone() {
          let maybe_rewritten =
            self.specifier_rewriter.rewrite(&lit_str.value, self.kind);
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
