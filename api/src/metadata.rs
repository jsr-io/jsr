// Copyright 2024 the JSR authors. All rights reserved. MIT license.
// https://www.notion.so/denolandinc/Deno-2-Roadmap-7301003f57754ccea043388d3cc15d8c
use crate::db::Database;
use crate::ids::PackageName;
use crate::ids::PackagePath;
use crate::ids::ScopeName;
use crate::ids::Version;
use indexmap::IndexMap;
use serde::Deserialize;
use serde::Serialize;
use std::collections::HashMap;

/// Looks like this:
/// ```json
/// {
///   "scope": "ry",
///   "name": "foo",
///   "versions": {
///     "0.1.2": {
///       "main": "./mod.ts"
///     },
///     "0.1.3": {
///       "main": "./mod.ts"
///     },
///   }
/// }
/// ```
/// See also [`gcs_paths::package_metadata`]
#[derive(Serialize, Deserialize)]
pub struct PackageMetadata {
  pub scope: ScopeName,
  pub name: PackageName,
  pub latest: Option<Version>,
  pub versions: HashMap<Version, PackageMetadataVersion>,
}

impl PackageMetadata {
  pub async fn create(
    db: &Database,
    scope: &ScopeName,
    package_name: &PackageName,
  ) -> anyhow::Result<Self> {
    let mut versions = db.list_package_versions(scope, package_name).await?;
    versions.sort_by(|(a, _), (b, _)| b.version.cmp(&a.version));
    let latest = versions
      .iter()
      .find(|(v, _)| !v.is_yanked && v.version.0.pre.is_empty())
      .map(|(v, _)| v.version.clone());
    let mut out = Self {
      scope: scope.to_owned(),
      name: package_name.to_owned(),
      latest,
      versions: HashMap::new(),
    };
    for (version, _) in versions {
      out.versions.insert(
        version.version,
        PackageMetadataVersion {
          yanked: version.is_yanked,
        },
      );
    }
    Ok(out)
  }
}

#[derive(Serialize, Deserialize)]
pub struct PackageMetadataVersion {
  #[serde(skip_serializing_if = "is_false", default)]
  pub yanked: bool,
}

/// This struct stores information specific to a particular published version.
/// We envision this to be a file manifest and in the future contain compilation
/// state that can help Deno run faster.
///
/// For example:
///
/// ```json
/// {
///  "main": "/mod.ts",
///  "moduleGraph": {
///    // includes cached info from
///    // https://github.com/denoland/deno_graph/blob/366ba1765e228f6db4121faf6ffa3c7ecf983779/src/analyzer.rs#L244
///    "mod.ts": {
///      "hash": "sha512-E1+My+HBCBHA6fBUZlbPnr...", // maybe...
///      // Contains deno_graph's ModuleInfo to make first runs faster.
///      // This information will only be used for the first run, and then
///      // after that it will used the cached information to allow the
///      // user to modify the cached file and have those changes reflected
///      "dependencies": [{
///        "specifier": "jsr:socket@2",
///        "specifierRange": [10, 23]
///      }]
///    },
///    "test.ts": {
///      "hash": "sha512-E1+My+HBCBHA6fBUZlbPnr...",
///      "dependencies": [{
///        "specifier": "jsr:assert",
///        "specifierRange": [20, 31]
///      }, {
///        "specifier": "./mod.ts",
///        "specifierRange": [80, 88]
///      }]
///    }
///  }
/// }
/// ```
/// See also [`gcs_paths::version_metadata`]
#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct VersionMetadata {
  pub manifest: HashMap<PackagePath, ManifestEntry>,
  pub module_graph_2: HashMap<String, deno_graph::ModuleInfo>,
  pub exports: IndexMap<String, String>,
}

#[derive(Serialize, Deserialize)]
pub struct ManifestEntry {
  pub size: usize,
  pub checksum: String,
}

fn is_false(b: &bool) -> bool {
  !b
}
