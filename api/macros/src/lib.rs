// Copyright 2024 the JSR authors. All rights reserved. MIT license.
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::OnceLock;

use proc_macro::TokenStream;
use quote::quote;
use syn::parse::{Parse, ParseStream};
use syn::{Expr, Ident, LitStr, Token, Type};

const FRAGMENTS_FILE: &str = "src/db/sql_fragments.rs";

fn load_fragments() -> &'static HashMap<String, String> {
  static MAP: OnceLock<HashMap<String, String>> = OnceLock::new();
  MAP.get_or_init(|| {
    let manifest_dir = std::env::var("CARGO_MANIFEST_DIR").unwrap();
    let path = PathBuf::from(&manifest_dir).join(FRAGMENTS_FILE);
    let content = std::fs::read_to_string(&path).unwrap();
    let file = syn::parse_file(&content).unwrap();

    let mut map = HashMap::new();
    for item in file.items {
      if let syn::Item::Const(item_const) = item
        && let syn::Expr::Lit(expr_lit) = &*item_const.expr
        && let syn::Lit::Str(lit_str) = &expr_lit.lit
      {
        map.insert(item_const.ident.to_string(), lit_str.value());
      }
    }
    map
  })
}

enum SqlFragment {
  Literal(String),
  Constant(Ident),
}

fn resolve_sql(fragments: &[SqlFragment]) -> syn::Result<(String, Vec<Ident>)> {
  let map = load_fragments();
  let mut parts = Vec::new();
  let mut idents = Vec::new();

  for fragment in fragments {
    match fragment {
      SqlFragment::Literal(s) => parts.push(s.as_str()),
      SqlFragment::Constant(ident) => {
        let name = ident.to_string();
        match map.get(&name) {
          Some(value) => {
            parts.push(value.as_str());
            idents.push(ident.clone());
          }
          None => {
            return Err(syn::Error::new(
              ident.span(),
              format!(
                "unknown SQL fragment `{name}`. \
                 Define it as a const in {FRAGMENTS_FILE}"
              ),
            ));
          }
        }
      }
    }
  }

  Ok((parts.concat(), idents))
}

struct QueryInput {
  fragments: Vec<SqlFragment>,
  binds: Vec<Expr>,
}

impl Parse for QueryInput {
  fn parse(input: ParseStream) -> syn::Result<Self> {
    let mut fragments = Vec::new();

    // Parse first fragment (required)
    fragments.push(parse_fragment(input)?);

    // Parse remaining comma-separated fragments, stop at `;` or EOF
    while input.peek(Token![,]) {
      input.parse::<Token![,]>()?;
      if input.is_empty() || input.peek(Token![;]) {
        break;
      }
      if input.peek(LitStr) || input.peek(Ident) {
        fragments.push(parse_fragment(input)?);
      } else {
        break;
      }
    }

    // Parse optional bind parameters after `;`
    let mut binds = Vec::new();
    if input.peek(Token![;]) {
      input.parse::<Token![;]>()?;
      while !input.is_empty() {
        binds.push(input.parse::<Expr>()?);
        if input.peek(Token![,]) {
          input.parse::<Token![,]>()?;
        } else {
          break;
        }
      }
    }

    Ok(QueryInput { fragments, binds })
  }
}

fn parse_fragment(input: ParseStream) -> syn::Result<SqlFragment> {
  if input.peek(LitStr) {
    let lit: LitStr = input.parse()?;
    Ok(SqlFragment::Literal(lit.value()))
  } else {
    let ident: Ident = input.parse()?;
    Ok(SqlFragment::Constant(ident))
  }
}

/// Like `sqlx::query!`, but concatenates multiple string literal and constant
/// fragments into a single SQL string.
///
/// Constants are resolved from `src/db/sql_fragments.rs` at compile time.
/// Bind parameters follow a `;`.
///
/// ```ignore
/// sqlx_query!("SELECT ", USER_SELECT, " FROM users WHERE id = $1"; &id)
/// ```
#[proc_macro]
pub fn query_concat(input: TokenStream) -> TokenStream {
  let QueryInput { fragments, binds } =
    syn::parse_macro_input!(input as QueryInput);

  let (sql, idents) = match resolve_sql(&fragments) {
    Ok(r) => r,
    Err(e) => return e.to_compile_error().into(),
  };

  let expanded = if binds.is_empty() {
    quote! {{
      #( let _ = crate::db::sql_fragments::#idents; )*
      sqlx::query!(#sql)
    }}
  } else {
    quote! {{
      #( let _ = crate::db::sql_fragments::#idents; )*
      sqlx::query!(#sql, #(#binds),*)
    }}
  };

  expanded.into()
}

struct QueryAsInput {
  ty: Type,
  fragments: Vec<SqlFragment>,
  binds: Vec<Expr>,
}

impl Parse for QueryAsInput {
  fn parse(input: ParseStream) -> syn::Result<Self> {
    let ty: Type = input.parse()?;
    input.parse::<Token![,]>()?;
    let QueryInput { fragments, binds } = input.parse()?;
    Ok(QueryAsInput {
      ty,
      fragments,
      binds,
    })
  }
}

/// Like `sqlx::query_as!`, but concatenates multiple string literal and
/// constant fragments into a single SQL string.
///
/// Constants are resolved from `src/db/sql_fragments.rs` at compile time.
/// Bind parameters follow a `;`.
///
/// ```ignore
/// sqlx_query_as!(User, "SELECT ", USER_SELECT, " FROM users WHERE id = $1"; &id)
/// ```
#[proc_macro]
pub fn query_concat_as(input: TokenStream) -> TokenStream {
  let QueryAsInput {
    ty,
    fragments,
    binds,
  } = syn::parse_macro_input!(input as QueryAsInput);

  let (sql, idents) = match resolve_sql(&fragments) {
    Ok(r) => r,
    Err(e) => return e.to_compile_error().into(),
  };

  let expanded = if binds.is_empty() {
    quote! {{
      #( let _ = crate::db::sql_fragments::#idents; )*
      sqlx::query_as!(#ty, #sql)
    }}
  } else {
    quote! {{
      #( let _ = crate::db::sql_fragments::#idents; )*
      sqlx::query_as!(#ty, #sql, #(#binds),*)
    }}
  };

  expanded.into()
}
