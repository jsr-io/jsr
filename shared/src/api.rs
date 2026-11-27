// Copyright 2024 the JSR authors. All rights reserved. MIT license.

use crate::models::*;
use serde::{Deserialize, Serialize};

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

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ApiPackageScore {
  pub has_readme: bool,
  pub has_readme_examples: bool,
  pub all_entrypoints_docs: bool,
  pub entrypoints_without_docs: Vec<String>,
  pub percentage_documented_symbols: f32,
  pub all_fast_check: bool,
  pub has_provenance: bool,

  // package wide
  pub has_description: bool,
  pub at_least_one_runtime_compatible: bool,
  pub multiple_runtimes_compatible: bool,

  pub total: u32,
}

impl ApiPackageScore {
  pub const MAX_SCORE: u32 = 17;

  pub fn score_percentage(&self) -> u32 {
    u32::min((self.total * 100) / Self::MAX_SCORE, 100)
  }
}

impl From<(&PackageVersionMeta, &Package)> for ApiPackageScore {
  fn from((meta, package): (&PackageVersionMeta, &Package)) -> Self {
    let mut score = 0;

    if meta.has_readme {
      score += 2;
    }

    if meta.has_readme_examples {
      score += 1;
    }

    if meta.all_entrypoints_docs {
      score += 1;
    }

    if meta.has_provenance {
      score += 1;
    }

    // You only need to document 80% of your symbols to get all the points.
    score += ((meta.percentage_documented_symbols / 0.8).min(1.0) * 5.0).floor()
      as u32;

    if meta.all_fast_check {
      score += 5;
    }

    // package wide

    if !package.description.is_empty() {
      score += 1;
    }

    let mut compatible_runtimes_count = 0;
    if package.runtime_compat.deno.is_some_and(|compat| compat) {
      compatible_runtimes_count += 1;
    }
    if package.runtime_compat.bun.is_some_and(|compat| compat) {
      compatible_runtimes_count += 1;
    }
    if package.runtime_compat.node.is_some_and(|compat| compat) {
      compatible_runtimes_count += 1;
    }
    if package.runtime_compat.browser.is_some_and(|compat| compat) {
      compatible_runtimes_count += 1;
    }
    if package.runtime_compat.workerd.is_some_and(|compat| compat) {
      compatible_runtimes_count += 1;
    }

    if compatible_runtimes_count >= 1 {
      score += 1;
    }

    if compatible_runtimes_count >= 2 {
      score += 1;
    }

    Self {
      has_readme: meta.has_readme,
      has_readme_examples: meta.has_readme_examples,
      all_entrypoints_docs: meta.all_entrypoints_docs,
      entrypoints_without_docs: meta.entrypoints_without_docs.clone(),
      percentage_documented_symbols: meta.percentage_documented_symbols,
      all_fast_check: meta.all_fast_check,
      has_provenance: meta.has_provenance,
      has_description: !package.description.is_empty(),
      at_least_one_runtime_compatible: compatible_runtimes_count >= 1,
      multiple_runtimes_compatible: compatible_runtimes_count >= 2,
      total: score,
    }
  }
}
