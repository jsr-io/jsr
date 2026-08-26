// Copyright 2024 the JSR authors. All rights reserved. MIT license.
//! Collect them all in one place for easy viewing.
use crate::ids::PackageName;
use crate::ids::PackagePath;
use crate::ids::ScopeName;
use crate::ids::Version;
use crate::npm::FIRST_NPM_LAYOUT_TARBALL_REVISION;
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

/// Storage path (relative to the npm bucket root, which doubles as the URL
/// path under `https://npm.jsr.io/`) of the npm compatibility tarball for
/// `revision`.
///
/// Since [`FIRST_NPM_LAYOUT_TARBALL_REVISION`], tarballs live at the path
/// layout used by registry.npmjs.org (`{name}/-/{basename}-{version}.tgz`),
/// because npm proxies such as JFrog Artifactory and Google Artifact Registry
/// construct tarball paths by that convention instead of reading
/// `dist.tarball` (https://github.com/jsr-io/jsr/issues/405). Earlier
/// revisions live under `~/{revision}/`; those objects are kept forever so
/// tarball URLs recorded in existing lockfiles keep resolving.
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
  if revision >= FIRST_NPM_LAYOUT_TARBALL_REVISION {
    format!("{npm_mapped_package_name}/-/{scope}__{package_name}-{version}.tgz")
  } else {
    format!("~/{revision}/{npm_mapped_package_name}/{version}.tgz")
  }
}

/// Public URL of an npm compatibility tarball, as advertised in the
/// `dist.tarball` field of the npm version manifest. Pass `npm_url` as
/// `https://npm.jsr.io/` (must end with a slash).
pub fn npm_tarball_url(
  npm_url: &url::Url,
  scope: &ScopeName,
  package_name: &PackageName,
  version: &Version,
  revision: u32,
) -> String {
  format!(
    "{npm_url}{}",
    npm_tarball_path(scope, package_name, version, revision)
  )
}

#[cfg(test)]
mod tests {
  use crate::ids::PackageName;
  use crate::ids::ScopeName;
  use crate::ids::Version;
  use crate::npm::FIRST_NPM_LAYOUT_TARBALL_REVISION;

  #[test]
  fn npm_tarball_path_layouts() {
    let scope = ScopeName::try_from("luca").unwrap();
    let package = PackageName::try_from("cases").unwrap();
    let version = Version::try_from("1.0.0").unwrap();

    // Legacy revisions keep the revisioned path so tarball URLs recorded in
    // existing lockfiles keep resolving.
    assert_eq!(
      super::npm_tarball_path(&scope, &package, &version, 11),
      "~/11/@jsr/luca__cases/1.0.0.tgz"
    );
    // Current revisions follow the registry.npmjs.org path layout.
    assert_eq!(
      super::npm_tarball_path(
        &scope,
        &package,
        &version,
        FIRST_NPM_LAYOUT_TARBALL_REVISION
      ),
      "@jsr/luca__cases/-/luca__cases-1.0.0.tgz"
    );

    let npm_url = url::Url::parse("https://npm.jsr.io/").unwrap();
    assert_eq!(
      super::npm_tarball_url(
        &npm_url,
        &scope,
        &package,
        &version,
        FIRST_NPM_LAYOUT_TARBALL_REVISION
      ),
      "https://npm.jsr.io/@jsr/luca__cases/-/luca__cases-1.0.0.tgz"
    );
  }

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
