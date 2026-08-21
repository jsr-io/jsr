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

pub fn docs_v2_path(
  scope: &ScopeName,
  package_name: &PackageName,
  version: &Version,
) -> String {
  format!("@{scope}/{package_name}/{version}/raw.rmp.gz")
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

/// Public URL of the package-level `meta.json` that the registry serves
/// to `deno install` / browser module resolution. Pass `registry_url`
/// as `https://jsr.io/` (must end with a slash).
pub fn package_metadata_url(
  registry_url: &url::Url,
  scope: &ScopeName,
  package_name: &PackageName,
) -> String {
  format!("{registry_url}@{scope}/{package_name}/meta.json")
}

/// Public URL of the npm version manifest the registry serves to
/// `npm install` / `pnpm install` / etc. Pass `npm_url` as
/// `https://npm.jsr.io/` (must end with a slash).
pub fn npm_version_manifest_url(
  npm_url: &url::Url,
  scope: &ScopeName,
  package_name: &PackageName,
) -> String {
  let npm_mapped_package_name = NpmMappedJsrPackageName {
    scope,
    package: package_name,
  };
  format!("{npm_url}{npm_mapped_package_name}")
}

/// Base URL of the public API host (`https://api.jsr.io/`), derived from the
/// registry URL (`https://jsr.io/`) by prefixing the host with `api.` — the two
/// always share a domain (see terraform `dns.tf`). Returns `None` if the host
/// can't be determined (e.g. a non-domain registry URL in local dev, where
/// cache purging is a no-op anyway).
fn api_base_url(registry_url: &url::Url) -> Option<String> {
  let host = registry_url.host_str()?;
  Some(format!("{}://api.{host}/", registry_url.scheme()))
}

/// Expand `paths` (each relative to the registry root, e.g.
/// `api/scopes/std/packages/foo`) into the set of fully-qualified URLs the lb
/// Worker caches them under. The lb keys its cache on the full request URL, and
/// the same endpoint is reachable — and separately cached — under both
/// `jsr.io/api/...` and `api.jsr.io/api/...`, so both are returned.
fn api_cache_urls(registry_url: &url::Url, paths: &[String]) -> Vec<String> {
  let api_base = api_base_url(registry_url);
  let mut urls = Vec::with_capacity(paths.len() * 2);
  for path in paths {
    urls.push(format!("{registry_url}{path}"));
    if let Some(api_base) = &api_base {
      urls.push(format!("{api_base}{path}"));
    }
  }
  urls
}

/// API endpoint URLs whose cached responses change when a version of
/// `@scope/name` is published, yanked, updated, or deleted. Pass `registry_url`
/// as `https://jsr.io/` (must end with a slash). Used to cache-bust the
/// aggressively-cached package endpoints (see `package_router`).
pub fn package_api_cache_urls(
  registry_url: &url::Url,
  scope: &ScopeName,
  package_name: &PackageName,
) -> Vec<String> {
  let pkg = format!("api/scopes/{scope}/packages/{package_name}");
  let paths = [
    pkg.clone(),
    format!("{pkg}/versions"),
    format!("{pkg}/versions/latest"),
    format!("{pkg}/versions/latest/docs"),
    format!("{pkg}/versions/latest/source"),
    format!("{pkg}/versions/latest/dependencies"),
    // Scope-level aggregates that surface this package and its latest version.
    format!("api/scopes/{scope}"),
    format!("api/scopes/{scope}/packages"),
  ];
  api_cache_urls(registry_url, &paths)
}

/// API endpoint URLs whose cached responses change when a package is created or
/// deleted within `scope`. Pass `registry_url` as `https://jsr.io/`.
pub fn scope_api_cache_urls(
  registry_url: &url::Url,
  scope: &ScopeName,
) -> Vec<String> {
  let paths = [
    format!("api/scopes/{scope}"),
    format!("api/scopes/{scope}/packages"),
  ];
  api_cache_urls(registry_url, &paths)
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
  use crate::ids::PackageName;
  use crate::ids::ScopeName;

  #[test]
  fn package_api_cache_urls_covers_both_hosts() {
    let registry_url = url::Url::parse("https://jsr.io/").unwrap();
    let scope = ScopeName::try_from("std").unwrap();
    let package = PackageName::try_from("fs").unwrap();
    let urls = super::package_api_cache_urls(&registry_url, &scope, &package);

    // Every path is purged under both jsr.io/api and api.jsr.io.
    assert!(urls.contains(&"https://jsr.io/api/scopes/std/packages/fs".into()));
    assert!(
      urls.contains(&"https://api.jsr.io/api/scopes/std/packages/fs".into())
    );
    assert!(urls.contains(
      &"https://jsr.io/api/scopes/std/packages/fs/versions/latest/docs".into()
    ));
    assert!(urls.contains(&"https://jsr.io/api/scopes/std".into()));
    assert!(urls.contains(&"https://api.jsr.io/api/scopes/std".into()));
  }

  #[test]
  fn version_metadata_is_correct() {
    let crazy = "= v 1.2.3-pre.other+build.test";
    // First show this crazy string actually parses.
    assert!(deno_semver::Version::parse_standard(crazy).is_ok());
    // but if we suffix a "_meta" it will not.
    assert!(
      deno_semver::Version::parse_standard(&format!("{crazy}_meta")).is_err()
    );
    assert!(
      deno_semver::Version::parse_standard(&format!("{crazy}_meta.json"))
        .is_err()
    );
    // Therefore /r/:scope/:package/:version_meta.json is ok.
  }
}
