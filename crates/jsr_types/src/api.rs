// Copyright 2024 the JSR authors. All rights reserved. MIT license.

//! Shared API wire types.
//!
//! These are the JSON response shapes served on `api.jsr.io`. They live in the
//! shared crate so both the Cloud Run compute service and the workers-rs front
//! serialize byte-identical responses (JSON parity), and so the Worker can
//! build them without depending on the native `api` crate. Only the wire types
//! that the Worker actually produces are moved here as their endpoints migrate;
//! the rest stay in `api/src/api/types.rs` for now.

use serde::Deserialize;
use serde::Serialize;

use crate::ids::PackageName;
use crate::ids::ScopeName;
use crate::ids::Version;
use crate::models::StatsPackage;
use crate::models::StatsPackageVersion;

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ApiStats {
  pub newest: Vec<ApiStatsPackage>,
  pub updated: Vec<ApiStatsPackageVersion>,
  pub featured: Vec<ApiStatsPackage>,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ApiStatsPackage {
  pub scope: ScopeName,
  pub name: PackageName,
}

impl From<StatsPackage> for ApiStatsPackage {
  fn from(p: StatsPackage) -> Self {
    Self {
      scope: p.scope,
      name: p.name,
    }
  }
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ApiStatsPackageVersion {
  pub scope: ScopeName,
  pub package: PackageName,
  pub version: Version,
}

impl From<StatsPackageVersion> for ApiStatsPackageVersion {
  fn from(v: StatsPackageVersion) -> Self {
    Self {
      scope: v.scope,
      package: v.name,
      version: v.version,
    }
  }
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ApiMetrics {
  pub packages: usize,
  pub packages_1d: usize,
  pub packages_7d: usize,
  pub packages_30d: usize,

  pub users: usize,
  pub users_1d: usize,
  pub users_7d: usize,
  pub users_30d: usize,

  pub package_versions: usize,
  pub package_versions_1d: usize,
  pub package_versions_7d: usize,
  pub package_versions_30d: usize,
}
