// Copyright 2024 the JSR authors. All rights reserved. MIT license.
use std::borrow::Cow;
use std::collections::HashMap;
use std::collections::HashSet;

use base64::Engine;
use deno_ast::apply_text_changes;
use deno_ast::SourceTextInfo;
use deno_ast::TextChange;
use deno_graph::analysis::DependencyDescriptor;
use deno_graph::analysis::ModuleAnalyzer;
use deno_graph::analysis::ModuleInfo;
use deno_graph::ast::CapturingModuleAnalyzer;
use deno_graph::ast::ParsedSourceStore;
use deno_graph::ModuleGraph;
use deno_graph::ModuleSpecifier;
use deno_graph::PositionRange;
use deno_graph::Resolution;
use deno_semver::package::PackageReqReference;
use futures::StreamExt;
use futures::TryStreamExt;
use indexmap::IndexMap;
use sha2::Digest;
use tar::Header;
use tracing::error;
use url::Url;

use crate::db::DependencyKind;
use crate::db::ExportsMap;
use crate::ids::PackageName;
use crate::ids::PackagePath;
use crate::ids::ScopeName;
use crate::ids::ScopedPackageName;
use crate::ids::Version;
use crate::s3::BucketWithQueue;

use super::emit::transpile_to_dts;
use super::emit::transpile_to_js;
use super::specifiers::follow_specifier;
use super::specifiers::relative_import_specifier;
use super::specifiers::rewrite_file_specifier;
use super::specifiers::Extension;
use super::specifiers::RewriteKind;
use super::specifiers::SpecifierRewriter;
use super::types::NpmExportConditions;
use super::types::NpmMappedJsrPackageName;
use super::types::NpmPackageJson;
use super::NPM_TARBALL_REVISION;

pub struct NpmTarball {
  /// The gzipped tarball contents.
  pub tarball: Vec<u8>,
  /// The hex encoded sha1 hash of the gzipped tarball.
  pub sha1: String,
  /// The base64 encoded sha512 hash of the gzipped tarball.
  pub sha512: String,
}

pub enum NpmTarballFiles<'a> {
  WithBytes(&'a HashMap<PackagePath, Vec<u8>>),
  FromBucket {
    files: &'a HashSet<PackagePath>,
    modules_bucket: &'a BucketWithQueue,
  },
}
