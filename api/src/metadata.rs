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
///       "yanked": true,
///       "createdAt": "2025-09-17T15:37:51.191487057Z"
///     },
///     "0.1.3": {
///       "createdAt": "2025-09-17T15:37:51.191487057Z"
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
          created_at: version.created_at,
        },
      );
    }
    Ok(out)
  }
}

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PackageMetadataVersion {
  #[serde(skip_serializing_if = "is_false", default)]
  pub yanked: bool,
  pub created_at: chrono::DateTime<chrono::Utc>,
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
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct VersionMetadata {
  pub manifest: HashMap<PackagePath, ManifestEntry>,
  pub module_graph_2: HashMap<String, deno_graph::analysis::ModuleInfo>,
  pub exports: IndexMap<String, String>,
}

impl<'de> Deserialize<'de> for VersionMetadata {
  fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
  where
    D: serde::Deserializer<'de>,
  {
    let mut value = serde_json::Value::deserialize(deserializer)
      .map_err(serde::de::Error::custom)?;
    let obj = value
      .as_object_mut()
      .ok_or_else(|| serde::de::Error::custom("expected object"))?;

    if !obj.contains_key("moduleGraph2")
      && let Some(mut module_graph_1) = obj.remove("moduleGraph1")
    {
      if let Some(graph_obj) = module_graph_1.as_object_mut() {
        for module_info in graph_obj.values_mut() {
          deno_graph::analysis::module_graph_1_to_2(module_info);
        }
      }
      obj.insert("moduleGraph2".to_string(), module_graph_1);
    }

    #[derive(Deserialize)]
    #[serde(rename_all = "camelCase")]
    struct Inner {
      manifest: HashMap<PackagePath, ManifestEntry>,
      module_graph_2: HashMap<String, deno_graph::analysis::ModuleInfo>,
      exports: IndexMap<String, String>,
    }

    let inner: Inner =
      serde_json::from_value(value).map_err(serde::de::Error::custom)?;
    Ok(VersionMetadata {
      manifest: inner.manifest,
      module_graph_2: inner.module_graph_2,
      exports: inner.exports,
    })
  }
}

#[derive(Serialize, Deserialize)]
pub struct ManifestEntry {
  pub size: usize,
  pub checksum: String,
}

fn is_false(b: &bool) -> bool {
  !b
}

#[cfg(test)]
mod tests {
  use super::*;

  fn base_json(module_graph_key: &str, graph_value: serde_json::Value) -> serde_json::Value {
    serde_json::json!({
      "manifest": {
        "/mod.ts": { "size": 100, "checksum": "sha256-abc" }
      },
      module_graph_key: graph_value,
      "exports": { ".": "./mod.ts" }
    })
  }

  fn simple_graph() -> serde_json::Value {
    serde_json::json!({
      "/mod.ts": {
        "dependencies": [{
          "type": "static",
          "kind": "import",
          "specifier": "./dep.ts",
          "specifierRange": [[1, 0], [1, 10]]
        }]
      }
    })
  }

  #[test]
  fn deserialize_module_graph_2() {
    let json = base_json("moduleGraph2", simple_graph());
    let meta: VersionMetadata = serde_json::from_value(json).unwrap();
    assert!(meta.module_graph_2.contains_key("/mod.ts"));
    assert_eq!(meta.module_graph_2["/mod.ts"].dependencies.len(), 1);
  }

  #[test]
  fn deserialize_module_graph_1_converts_to_2() {
    let graph = serde_json::json!({
      "/mod.ts": {
        "dependencies": [{
          "type": "static",
          "kind": "import",
          "specifier": "./a.js",
          "specifierRange": [[1, 0], [1, 10]],
          "leadingComments": [{
            "text": " @deno-types=\"./a.d.ts\"",
            "range": [[0, 0], [0, 25]]
          }]
        }]
      }
    });
    let json = base_json("moduleGraph1", graph);
    let meta: VersionMetadata = serde_json::from_value(json).unwrap();
    assert!(meta.module_graph_2.contains_key("/mod.ts"));
    let dep = &meta.module_graph_2["/mod.ts"].dependencies[0];
    // After conversion, leadingComments should be gone and
    // typesSpecifier should be populated instead.
    let static_dep = dep.as_static().expect("expected static dependency");
    assert!(static_dep.types_specifier.is_some());
  }

  #[test]
  fn deserialize_module_graph_2_takes_precedence_over_1() {
    let mut json = base_json("moduleGraph2", simple_graph());
    // Also inject a moduleGraph1 key — it should be ignored.
    json.as_object_mut().unwrap().insert(
      "moduleGraph1".to_string(),
      serde_json::json!({ "/ignored.ts": {} }),
    );
    let meta: VersionMetadata = serde_json::from_value(json).unwrap();
    assert!(meta.module_graph_2.contains_key("/mod.ts"));
    assert!(!meta.module_graph_2.contains_key("/ignored.ts"));
  }

  #[test]
  fn serialize_always_uses_module_graph_2() {
    let json = base_json("moduleGraph2", simple_graph());
    let meta: VersionMetadata = serde_json::from_value(json).unwrap();
    let serialized = serde_json::to_value(&meta).unwrap();
    assert!(serialized.get("moduleGraph2").is_some());
    assert!(serialized.get("moduleGraph1").is_none());
  }
}
