// Copyright 2024 the JSR authors. All rights reserved. MIT license.
use chrono::SecondsFormat;
use deno_semver::StackString;
use deno_semver::VersionReq;
use deno_semver::package::PackageReq;
use deno_semver::package::PackageReqReference;
use indexmap::IndexMap;
use serde::Serialize;
use std::borrow::Cow;
use std::collections::HashMap;
use url::Url;

use crate::db::Database;
use crate::ids::PackageName;
use crate::ids::ScopeName;
use crate::ids::ScopedPackageName;
use crate::ids::Version;
use crate::models::DependencyKind;
use crate::models::PackageVersionDependency;

pub const NPM_TARBALL_REVISION: u32 = 11;

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

#[derive(Debug, Clone, Serialize)]
pub struct NpmRepositoryInfo {
  #[serde(rename = "type")]
  pub repository_type: String,
  pub url: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct NpmVersionInfo<'a> {
  pub name: NpmMappedJsrPackageName<'a>,
  pub version: Version,
  pub description: String,
  #[serde(skip_serializing_if = "Option::is_none")]
  pub repository: Option<NpmRepositoryInfo>,
  pub dist: NpmDistInfo,
  pub dependencies: IndexMap<String, String>,
}

#[derive(Debug, Serialize)]
pub struct NpmPackageInfo<'a> {
  pub name: NpmMappedJsrPackageName<'a>,
  pub description: String,
  #[serde(skip_serializing_if = "Option::is_none")]
  pub repository: Option<NpmRepositoryInfo>,
  #[serde(rename = "dist-tags")]
  pub dist_tags: IndexMap<String, Version>,
  pub versions: IndexMap<Version, NpmVersionInfo<'a>>,
  // Used by `npm show <package>`
  pub time: IndexMap<String, String>,
}

#[derive(Debug, Serialize)]
pub struct NpmExportConditions {
  #[serde(skip_serializing_if = "Option::is_none")]
  pub types: Option<String>,
  #[serde(skip_serializing_if = "Option::is_none")]
  pub default: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct NpmPackageJson<'a> {
  pub name: NpmMappedJsrPackageName<'a>,
  pub version: Version,
  pub homepage: String,

  #[serde(rename = "type")]
  pub module_type: String,
  pub dependencies: IndexMap<String, String>,
  pub exports: IndexMap<String, NpmExportConditions>,

  #[serde(rename = "_jsr_revision")]
  pub revision: u32,
}

pub fn create_npm_dependencies<'a>(
  dependencies: impl Iterator<Item = Cow<'a, (DependencyKind, PackageReqReference)>>,
) -> Result<IndexMap<String, String>, anyhow::Error> {
  let mut npm_dependencies = IndexMap::new();
  for dep in dependencies {
    let (kind, req) = &*dep;
    match kind {
      DependencyKind::Jsr => {
        let jsr_name = ScopedPackageName::new(req.req.name.to_string())?;
        let npm_name = NpmMappedJsrPackageName {
          scope: &jsr_name.scope,
          package: &jsr_name.package,
        };
        npm_dependencies
          .insert(npm_name.to_string(), req.req.version_req.to_string());
      }
      DependencyKind::Npm => {
        npm_dependencies
          .insert(req.req.name.to_string(), req.req.version_req.to_string());
      }
    }
  }
  npm_dependencies.sort_keys();
  Ok(npm_dependencies)
}

pub async fn generate_npm_version_manifest<'a>(
  db: &Database,
  npm_url: &Url,
  scope: &'a ScopeName,
  name: &'a PackageName,
) -> Result<NpmPackageInfo<'a>, anyhow::Error> {
  let (package, github_repository, _) = db
    .get_package(scope, name)
    .await?
    .ok_or_else(|| anyhow::anyhow!("package not found: @{scope}/{name}"))?;

  let repository = github_repository.map(|repo| NpmRepositoryInfo {
    repository_type: "git".to_string(),
    url: format!("git+https://github.com/{}/{}.git", repo.owner, repo.name),
  });

  let versions = db
    .list_package_versions_for_npm_version_manifest(scope, name)
    .await?;

  let all_dependencies = db.list_package_dependencies(scope, name).await?;

  let mut dependencies_per_version: HashMap<
    Version,
    Vec<PackageVersionDependency>,
  > = HashMap::new();

  for dep in all_dependencies {
    dependencies_per_version
      .entry(dep.package_version.clone())
      .or_default()
      .push(dep);
  }

  let mut out = NpmPackageInfo {
    name: NpmMappedJsrPackageName {
      scope,
      package: name,
    },
    description: package.description.clone(),
    repository: repository.clone(),
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

  for version in versions {
    // We don't publish yanked versions in the NPM manifest.
    if version.is_yanked {
      continue;
    }

    let dependencies = dependencies_per_version
      .remove(&version.version)
      .unwrap_or_default();

    let dependencies = dependencies.into_iter().map(|dep| {
      let sub_path = if dep.dependency_path.is_empty() {
        None
      } else {
        Some(deno_semver::package::PackageSubPath::from_string(
          dep.dependency_path,
        ))
      };
      let version_req =
        VersionReq::parse_from_specifier(&dep.dependency_constraint).unwrap();
      let req = PackageReq {
        name: StackString::from_string(dep.dependency_name),
        version_req,
      };
      Cow::Owned((dep.dependency_kind, PackageReqReference { req, sub_path }))
    });
    let npm_dependencies = create_npm_dependencies(dependencies)?;

    let tarball = Url::options()
      .base_url(Some(npm_url))
      .parse(&format!(
        "./~/{}/{}/{}.tgz",
        version.npm_tarball_revision,
        NpmMappedJsrPackageName {
          scope,
          package: name,
        },
        version.version,
      ))
      .unwrap();

    let npm_version_info = NpmVersionInfo {
      name: NpmMappedJsrPackageName {
        scope,
        package: name,
      },
      version: version.version.clone(),
      description: package.description.clone(),
      repository: repository.clone(),
      dist: NpmDistInfo {
        tarball: tarball.to_string(),
        shasum: version.npm_tarball_sha1,
        integrity: format!("sha512-{}", version.npm_tarball_sha512),
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
