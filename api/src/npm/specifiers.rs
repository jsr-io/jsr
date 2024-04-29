// Copyright 2024 the JSR authors. All rights reserved. MIT license.

use std::borrow::Cow;
use std::collections::HashMap;

use deno_ast::ModuleSpecifier;
use deno_graph::Dependency;
use deno_semver::jsr::JsrPackageReqReference;
use deno_semver::npm::NpmPackageReqReference;
use indexmap::IndexMap;

use crate::ids::ScopedPackageName;

use super::NpmMappedJsrPackageName;

#[derive(Clone, Copy)]
pub enum RewriteKind {
  Source,
  Declaration,
}

#[derive(Clone, Copy)]
pub struct SpecifierRewriter<'a> {
  pub base_specifier: &'a ModuleSpecifier,
  pub source_rewrites: &'a HashMap<&'a ModuleSpecifier, ModuleSpecifier>,
  pub declaration_rewrites: &'a HashMap<&'a ModuleSpecifier, ModuleSpecifier>,
  pub dependencies: &'a IndexMap<String, Dependency>,
}

impl<'a> SpecifierRewriter<'a> {
  pub fn rewrite(&self, specifier: &str, kind: RewriteKind) -> Option<String> {
    let dep = self.dependencies.get(specifier)?;

    let specifier = match kind {
      RewriteKind::Source => dep.get_code(),
      RewriteKind::Declaration => dep.get_type().or_else(|| dep.get_code()),
    }?;

    let rewrites = match kind {
      RewriteKind::Source => self.source_rewrites,
      RewriteKind::Declaration => self.declaration_rewrites,
    };

    let mut resolved_specifier =
      Cow::Borrowed(follow_specifier(specifier, rewrites)?);

    if let Some(specifier) =
      rewrite_npm_and_jsr_specifier(resolved_specifier.as_str())
    {
      return Some(specifier);
    };

    if matches!(kind, RewriteKind::Declaration)
      && resolved_specifier.scheme() == "file"
    {
      let path = resolved_specifier.path();
      if path.ends_with(".d.ts") || path.ends_with(".d.mts") {
        // If the base specifier is a declaration file, and a dependency is also a
        // declaration file, TypeScript will not allow the import (TS2846). In
        // this case, replace the `.d.ts` extension in the resolved specifier
        // with `.js` so that TypeScript thinks we're importing a source file,
        // which is allowed. It will then probe for the `.d.ts` file, which it
        // will find.
        // We do not use extensionless imports, because TypeScript does not
        // allow them under `moduleResolution: "nodenext"` (TS2835).
        let path = rewrite_path_extension(path, Extension::Js).unwrap();
        resolved_specifier.to_mut().set_path(&path);
      }
    }

    if *resolved_specifier == *specifier {
      // No need to rewrite if the specifier is the same as the resolved
      // specifier.
      return None;
    }

    let new_specifier = if resolved_specifier.scheme() == "file" {
      relative_import_specifier(self.base_specifier, &resolved_specifier)
    } else {
      resolved_specifier.to_string()
    };

    Some(new_specifier)
  }
}

pub fn relative_import_specifier(
  base_specifier: &ModuleSpecifier,
  specifier: &ModuleSpecifier,
) -> String {
  let relative = base_specifier.make_relative(specifier).unwrap();
  if relative.is_empty() {
    format!("./{}", specifier.path_segments().unwrap().last().unwrap())
  } else if relative.starts_with("../") {
    relative.to_string()
  } else {
    format!("./{}", relative)
  }
}

pub fn follow_specifier<'a>(
  specifier: &'a ModuleSpecifier,
  remapped_specifiers: &'a HashMap<&ModuleSpecifier, ModuleSpecifier>,
) -> Option<&'a ModuleSpecifier> {
  let mut redirects = 0;
  let mut specifier = specifier;
  loop {
    // avoid infinite loops
    if redirects > 10 {
      return None;
    }
    if let Some(rewritten) = remapped_specifiers.get(&specifier) {
      specifier = rewritten;
    } else {
      break;
    }
    redirects += 1;
  }
  Some(specifier)
}

pub fn rewrite_npm_and_jsr_specifier(specifier: &str) -> Option<String> {
  if let Ok(jsr) = JsrPackageReqReference::from_str(specifier) {
    let req = jsr.into_inner();
    let jsr_name = ScopedPackageName::new(req.req.name).ok()?;
    let npm_name = NpmMappedJsrPackageName {
      scope: &jsr_name.scope,
      package: &jsr_name.package,
    };
    // TODO: also check package version - there may be duplicate requests for
    // the same package
    let rewritten = format!(
      "{}{}",
      npm_name,
      match &req.sub_path {
        Some(subpath) => format!("/{}", subpath),
        None => "".to_owned(),
      }
    );
    Some(rewritten)
  } else if let Ok(npm) = NpmPackageReqReference::from_str(specifier) {
    let req = npm.into_inner();
    // TODO: also check package version - there may be duplicate requests for
    // the same package
    let rewritten = format!(
      "{}{}",
      req.req.name,
      match &req.sub_path {
        Some(subpath) => format!("/{}", subpath),
        None => "".to_owned(),
      }
    );
    Some(rewritten)
  } else {
    None
  }
}

pub enum Extension {
  Js,
  #[allow(dead_code)]
  Dts,
}

pub fn rewrite_file_specifier(
  specifier: &ModuleSpecifier,
  prefix: &str,
  new_extension: Extension,
) -> Option<ModuleSpecifier> {
  assert_eq!(specifier.scheme(), "file");
  let path = specifier.path();
  let rewritten_path = rewrite_path_extension(path, new_extension)?;
  Some(
    ModuleSpecifier::parse(&format!("file://{prefix}{rewritten_path}"))
      .unwrap(),
  )
}

pub fn rewrite_path_extension(
  path: &str,
  new_extension: Extension,
) -> Option<String> {
  let (basename, name) = path.rsplit_once('/')?;
  let (name, ext) = if let Some(name) = name.strip_suffix(".d.ts") {
    (name, "d.ts")
  } else if let Some(name) = name.strip_suffix(".d.mts") {
    (name, "d.mts")
  } else {
    name.rsplit_once('.')?
  };
  let new_ext = match new_extension {
    Extension::Js => "js",
    Extension::Dts => "d.ts",
  };
  if ext == new_ext {
    return None;
  }
  Some(format!("{}/{}.{}", basename, name, new_ext))
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn test_rewrite_specifier_jsr() {
    assert_eq!(
      rewrite_npm_and_jsr_specifier("jsr:@std/fs"),
      Some("@jsr/std__fs".to_owned())
    );
    assert_eq!(
      rewrite_npm_and_jsr_specifier("jsr:@std/fs/file_server"),
      Some("@jsr/std__fs/file_server".to_owned())
    );
    assert_eq!(
      rewrite_npm_and_jsr_specifier("jsr:@std/fs@0.0.1"),
      Some("@jsr/std__fs".to_owned())
    );
    assert_eq!(
      rewrite_npm_and_jsr_specifier("jsr:@std/fs@0.0.1/file_server"),
      Some("@jsr/std__fs/file_server".to_owned())
    );
  }

  #[test]
  fn test_rewrite_specifier_npm() {
    assert_eq!(
      rewrite_npm_and_jsr_specifier("npm:@std/fs"),
      Some("@std/fs".to_owned())
    );
    assert_eq!(
      rewrite_npm_and_jsr_specifier("npm:@std/fs/file_server"),
      Some("@std/fs/file_server".to_owned())
    );
    assert_eq!(
      rewrite_npm_and_jsr_specifier("npm:@std/fs@0.0.1"),
      Some("@std/fs".to_owned())
    );
    assert_eq!(
      rewrite_npm_and_jsr_specifier("npm:@std/fs@0.0.1/file_server"),
      Some("@std/fs/file_server".to_owned())
    );
    assert_eq!(
      rewrite_npm_and_jsr_specifier("npm:express"),
      Some("express".to_owned())
    );
    assert_eq!(
      rewrite_npm_and_jsr_specifier("npm:express/file_server"),
      Some("express/file_server".to_owned())
    );
    assert_eq!(
      rewrite_npm_and_jsr_specifier("npm:express@0.0.1"),
      Some("express".to_owned())
    );
    assert_eq!(
      rewrite_npm_and_jsr_specifier("npm:express@0.0.1/file_server"),
      Some("express/file_server".to_owned())
    );
  }

  #[test]
  fn test_rewrite_path_extension() {
    assert_eq!(
      rewrite_path_extension("foo/bar.ts", Extension::Js),
      Some("foo/bar.js".to_owned())
    );
    assert_eq!(
      rewrite_path_extension("foo/bar.ts", Extension::Dts),
      Some("foo/bar.d.ts".to_owned())
    );
    assert_eq!(
      rewrite_path_extension("foo/bar.d.ts", Extension::Js),
      Some("foo/bar.js".to_owned())
    );
    assert_eq!(rewrite_path_extension("foo/bar.d.ts", Extension::Dts), None);
    assert_eq!(
      rewrite_path_extension("foo/bar.d.mts", Extension::Js),
      Some("foo/bar.js".to_owned())
    );
    assert_eq!(
      rewrite_path_extension("foo/bar.d.mts", Extension::Dts),
      Some("foo/bar.d.ts".to_owned())
    );
    assert_eq!(rewrite_path_extension("foo/bar.js", Extension::Js), None);
    assert_eq!(
      rewrite_path_extension("foo/bar.js", Extension::Dts),
      Some("foo/bar.d.ts".to_owned())
    );
    assert_eq!(
      rewrite_path_extension("foo/bar.jsx", Extension::Js),
      Some("foo/bar.js".to_owned())
    );
    assert_eq!(
      rewrite_path_extension("foo/bar.jsx", Extension::Dts),
      Some("foo/bar.d.ts".to_owned())
    );
  }

  #[test]
  fn test_relative_import_specifier() {
    assert_eq!(
      relative_import_specifier(
        &ModuleSpecifier::parse("file:///a/b/c.ts").unwrap(),
        &ModuleSpecifier::parse("file:///a/b/d.ts").unwrap(),
      ),
      "./d.ts",
    );
    assert_eq!(
      relative_import_specifier(
        &ModuleSpecifier::parse("file:///a/b/c.ts").unwrap(),
        &ModuleSpecifier::parse("file:///a/d.ts").unwrap(),
      ),
      "../d.ts",
    );
    assert_eq!(
      relative_import_specifier(
        &ModuleSpecifier::parse("file:///a/b/c.ts").unwrap(),
        &ModuleSpecifier::parse("file:///a/b/c.ts").unwrap(),
      ),
      "./c.ts",
    );
    assert_eq!(
      relative_import_specifier(
        &ModuleSpecifier::parse("file:///a/b/c.ts").unwrap(),
        &ModuleSpecifier::parse("file:///a/b/c/d.ts").unwrap(),
      ),
      "./c/d.ts",
    );
  }
}
