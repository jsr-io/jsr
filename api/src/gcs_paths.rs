// Copyright 2024 the JSR authors. All rights reserved. MIT license.
//! Collect them all in one place for easy viewing.
use crate::ids::PackageName;
use crate::ids::PackagePath;
use crate::ids::ScopeName;
use crate::ids::Version;
use crate::npm::NpmMappedJsrPackageName;

pub fn file_path(
  scope: &ScopeName,
  package_name: &PackageName,
  version: &Version,
  path: &PackagePath,
) -> String {
  format!("@{scope}/{package_name}/{version}{path}")
}

pub fn file_path_root_directory(
  scope: &ScopeName,
  package_name: &PackageName,
  version: &Version,
) -> String {
  format!("@{scope}/{package_name}/{version}/")
}

pub fn docs_v1_path(
  scope: &ScopeName,
  package_name: &PackageName,
  version: &Version,
) -> String {
  format!("@{scope}/{package_name}/{version}/raw.json")
}

pub fn package_metadata(
  scope: &ScopeName,
  package_name: &PackageName,
) -> String {
  format!("@{scope}/{package_name}/meta.json")
}

#[allow(dead_code)]
pub fn top_level_package_metadata(package_name: &PackageName) -> String {
  format!("{package_name}/meta.json")
}

pub fn version_metadata(
  scope: &ScopeName,
  package_name: &PackageName,
  version: &Version,
) -> String {
  format!("@{scope}/{package_name}/{version}_meta.json")
}

pub fn npm_version_manifest_path(
  scope: &ScopeName,
  package_name: &PackageName,
) -> String {
  let npm_mapped_package_name = NpmMappedJsrPackageName {
    scope,
    package: package_name,
  };
  format!("{npm_mapped_package_name}")
}

pub fn npm_tarball_path(
  scope: &ScopeName,
  package_name: &PackageName,
  version: &Version,
  revision: u32,
) -> String {
  let npm_mapped_package_name = NpmMappedJsrPackageName {
    scope,
    package: package_name,
  };
  format!("~/{revision}/{npm_mapped_package_name}/{version}.tgz")
}

#[cfg(test)]
mod tests {

  #[test]
  fn version_metadata_is_correct() {
    let crazy = "= v 1.2.3-pre.other+build.test";
    // First show this crazy string actually parses.
    assert!(deno_semver::Version::parse_standard(crazy).is_ok());
    // but if we suffix a "_meta" it will not.
    assert!(
      deno_semver::Version::parse_standard(&format!("{crazy}_meta")).is_err()
    );
    assert!(deno_semver::Version::parse_standard(&format!(
      "{crazy}_meta.json"
    ))
    .is_err());
    // Therefore /r/:scope/:package/:version_meta.json is ok.
  }
}
