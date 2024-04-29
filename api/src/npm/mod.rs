// Copyright 2024 the JSR authors. All rights reserved. MIT license.
mod emit;
mod import_transform;
mod specifiers;
mod tarball;
#[cfg(test)]
mod tests;
mod types;

use chrono::SecondsFormat;
use deno_semver::package::PackageReq;
use deno_semver::package::PackageReqReference;
use deno_semver::VersionReq;
use indexmap::IndexMap;
use std::borrow::Cow;
use url::Url;

use crate::db::Database;
use crate::ids::PackageName;
use crate::ids::ScopeName;
use crate::npm::tarball::create_npm_dependencies;
use crate::npm::types::NpmDistInfo;
use crate::npm::types::NpmPackageInfo;

pub use self::tarball::create_npm_tarball;
pub use self::tarball::NpmTarball;
pub use self::tarball::NpmTarballFiles;
pub use self::tarball::NpmTarballOptions;
pub use self::types::NpmMappedJsrPackageName;
use self::types::NpmVersionInfo;

pub const NPM_TARBALL_REVISION: u32 = 10;

pub async fn generate_npm_version_manifest<'a>(
  db: &Database,
  npm_url: &Url,
  scope: &'a ScopeName,
  name: &'a PackageName,
) -> Result<NpmPackageInfo<'a>, anyhow::Error> {
  let (package, _, _) = db
    .get_package(scope, name)
    .await?
    .ok_or_else(|| anyhow::anyhow!("package not found: @{scope}/{name}"))?;

  let versions = db.list_package_versions(scope, name).await?;

  let mut out = NpmPackageInfo {
    name: NpmMappedJsrPackageName {
      scope,
      package: name,
    },
    description: package.description.clone(),
    dist_tags: IndexMap::new(),
    versions: IndexMap::new(),
    time: IndexMap::new(),
  };

  out.time.insert(
    "created".to_string(),
    package
      .created_at
      .to_rfc3339_opts(SecondsFormat::Millis, true),
  );
  out.time.insert(
    "modified".to_string(),
    package
      .updated_at
      .to_rfc3339_opts(SecondsFormat::Millis, true),
  );

  for (version, _) in versions {
    // We don't publish yanked versions in the NPM manifest.
    if version.is_yanked {
      continue;
    }

    // Skip versions that don't have a tarball.
    let Some(npm_tarball) = db
      .get_latest_npm_tarball_for_version(scope, name, &version.version)
      .await?
    else {
      continue;
    };

    let dependencies = db
      .list_package_version_dependencies(scope, name, &version.version)
      .await?;
    let dependencies = dependencies.into_iter().map(|dep| {
      let sub_path = if dep.dependency_path.is_empty() {
        None
      } else {
        Some(dep.dependency_path)
      };
      let version_req =
        VersionReq::parse_from_specifier(&dep.dependency_constraint).unwrap();
      let req = PackageReq {
        name: dep.dependency_name,
        version_req,
      };
      Cow::Owned((dep.dependency_kind, PackageReqReference { req, sub_path }))
    });
    let npm_dependencies = create_npm_dependencies(dependencies)?;

    let tarball = Url::options()
      .base_url(Some(npm_url))
      .parse(&format!(
        "./~/{}/{}/{}.tgz",
        npm_tarball.revision,
        NpmMappedJsrPackageName {
          scope,
          package: name,
        },
        &version.version,
      ))
      .unwrap();

    let npm_version_info = NpmVersionInfo {
      name: NpmMappedJsrPackageName {
        scope,
        package: name,
      },
      version: version.version.clone(),
      description: package.description.clone(),
      dist: NpmDistInfo {
        tarball: tarball.to_string(),
        shasum: npm_tarball.sha1,
        integrity: format!("sha512-{}", npm_tarball.sha512),
      },
      dependencies: npm_dependencies,
    };

    out
      .versions
      .insert(version.version.clone(), npm_version_info);
    out.time.insert(
      version.version.to_string(),
      version
        .created_at
        .to_rfc3339_opts(SecondsFormat::Millis, true),
    );
  }

  if let Some((version, _)) = out.versions.first() {
    out.dist_tags.insert("latest".to_string(), version.clone());
  }

  Ok(out)
}
