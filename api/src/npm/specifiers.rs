// Copyright 2024 the JSR authors. All rights reserved. MIT license.
use deno_semver::jsr::JsrPackageReqReference;
use deno_semver::npm::NpmPackageReqReference;

use crate::ids::ScopedPackageName;

use super::NpmMappedJsrPackageName;

pub fn rewrite_specifier(specifier: &str) -> Option<String> {
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
  } else if specifier.starts_with("./") || specifier.starts_with("../") {
    rewrite_extension(specifier, Extension::Js)
  } else {
    None
  }
}

pub enum Extension {
  Js,
  #[allow(dead_code)]
  Dts,
}

pub fn rewrite_extension(path: &str, new_ext: Extension) -> Option<String> {
  let (basename, name) = path.rsplit_once('/')?;
  if name.ends_with(".d.ts") {
    return None;
  }
  let (name, ext) = name.rsplit_once('.')?;
  match new_ext {
    Extension::Js => match ext {
      "ts" | "tsx" | "jsx" => Some(format!("{}/{}.js", basename, name)),
      _ => None,
    },
    Extension::Dts => Some(format!("{}/{}.d.ts", basename, name)),
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn test_rewrite_specifier_jsr() {
    assert_eq!(
      rewrite_specifier("jsr:@std/fs"),
      Some("@jsr/std__fs".to_owned())
    );
    assert_eq!(
      rewrite_specifier("jsr:@std/fs/file_server"),
      Some("@jsr/std__fs/file_server".to_owned())
    );
    assert_eq!(
      rewrite_specifier("jsr:@std/fs@0.0.1"),
      Some("@jsr/std__fs".to_owned())
    );
    assert_eq!(
      rewrite_specifier("jsr:@std/fs@0.0.1/file_server"),
      Some("@jsr/std__fs/file_server".to_owned())
    );
  }

  #[test]
  fn test_rewrite_specifier_npm() {
    assert_eq!(rewrite_specifier("npm:@std/fs"), Some("@std/fs".to_owned()));
    assert_eq!(
      rewrite_specifier("npm:@std/fs/file_server"),
      Some("@std/fs/file_server".to_owned())
    );
    assert_eq!(
      rewrite_specifier("npm:@std/fs@0.0.1"),
      Some("@std/fs".to_owned())
    );
    assert_eq!(
      rewrite_specifier("npm:@std/fs@0.0.1/file_server"),
      Some("@std/fs/file_server".to_owned())
    );
    assert_eq!(rewrite_specifier("npm:express"), Some("express".to_owned()));
    assert_eq!(
      rewrite_specifier("npm:express/file_server"),
      Some("express/file_server".to_owned())
    );
    assert_eq!(
      rewrite_specifier("npm:express@0.0.1"),
      Some("express".to_owned())
    );
    assert_eq!(
      rewrite_specifier("npm:express@0.0.1/file_server"),
      Some("express/file_server".to_owned())
    );
  }

  #[test]
  fn test_rewrite_specifier_relative() {
    assert_eq!(
      rewrite_specifier("./foo/bar.ts"),
      Some("./foo/bar.js".to_owned())
    );
    assert_eq!(
      rewrite_specifier("../foo.tsx"),
      Some("../foo.js".to_owned())
    );
    assert_eq!(
      rewrite_specifier("../foo.jsx"),
      Some("../foo.js".to_owned())
    );
    assert_eq!(rewrite_specifier("./foo.js"), None);
    assert_eq!(rewrite_specifier("./foo.d.ts"), None);
  }
}
