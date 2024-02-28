// Copyright 2024 the JSR authors. All rights reserved. MIT license.
use indexmap::IndexMap;
use serde::Serialize;

use crate::ids::PackageName;
use crate::ids::ScopeName;
use crate::ids::Version;

// TODO: We don't have the @jsr scope on npm
pub const NPM_SCOPE: &str = "jsr";

pub struct NpmMappedJsrPackageName<'a> {
  pub scope: &'a ScopeName,
  pub package: &'a PackageName,
}

impl std::fmt::Display for NpmMappedJsrPackageName<'_> {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    write!(f, "@{NPM_SCOPE}/{}__{}", self.scope, self.package)
  }
}

impl std::fmt::Debug for NpmMappedJsrPackageName<'_> {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    write!(f, "@{NPM_SCOPE}/{}__{}", self.scope, self.package)
  }
}

impl serde::Serialize for NpmMappedJsrPackageName<'_> {
  fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
  where
    S: serde::Serializer,
  {
    String::serialize(&format!("{self}"), serializer)
  }
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct NpmDistInfo {
  pub tarball: String,
  pub shasum: String,
  pub integrity: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct NpmVersionInfo<'a> {
  pub name: NpmMappedJsrPackageName<'a>,
  pub version: Version,
  pub description: String,
  pub dist: NpmDistInfo,
  pub dependencies: IndexMap<String, String>,
}

#[derive(Debug, Serialize)]
pub struct NpmPackageInfo<'a> {
  pub name: NpmMappedJsrPackageName<'a>,
  pub description: String,
  #[serde(rename = "dist-tags")]
  pub dist_tags: IndexMap<String, Version>,
  pub versions: IndexMap<Version, NpmVersionInfo<'a>>,
  // Used by `npm show <package>`
  pub time: IndexMap<String, String>,
}

#[derive(Debug, Serialize)]
pub struct NpmPackageJson<'a> {
  pub name: NpmMappedJsrPackageName<'a>,
  pub version: Version,
  pub homepage: String,

  #[serde(rename = "type")]
  pub module_type: String,
  pub dependencies: IndexMap<String, String>,
  pub exports: IndexMap<String, String>,
}
