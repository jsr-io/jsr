// Copyright 2024 the JSR authors. All rights reserved. MIT license.
use std::collections::HashMap;
use std::collections::HashSet;
use std::io;
use std::sync::OnceLock;

use async_tar::EntryType;
use bytes::Bytes;
use deno_ast::MediaType;
use deno_graph::ModuleGraphError;
use deno_semver::VersionReq;
use deno_semver::jsr::JsrPackageReqReference;
use deno_semver::npm::NpmPackageReqReference;
use deno_semver::package::PackageReq;
use deno_semver::package::PackageReqReference;
use deno_semver::package::PackageReqReferenceParseError;
use futures::AsyncReadExt;
use futures::StreamExt;
use futures::TryStreamExt;
use indexmap::IndexMap;
use jsonc_parser::ParseOptions;
use reqwest::StatusCode;
use serde::Deserialize;
use serde::Serialize;
use sha2::Digest;
use thiserror::Error;
use tracing::Span;
use tracing::instrument;
use url::Url;
use uuid::Uuid;

use crate::analysis::PackageAnalysisData;
use crate::analysis::PackageAnalysisOutput;
use crate::analysis::analyze_package;
use crate::db::Database;
use crate::db::ExportsMap;
use crate::db::PublishingTask;
use crate::db::{DependencyKind, PackageVersionMeta};
use crate::ids::CaseInsensitivePackagePath;
use crate::ids::PackageName;
use crate::ids::PackagePath;
use crate::ids::PackagePathValidationError;
use crate::ids::ScopeName;
use crate::ids::ScopedPackageName;
use crate::ids::ScopedPackageNameValidateError;
use crate::ids::Version;
use crate::metadata::PackageMetadata;
use crate::metadata::VersionMetadata;
use crate::npm::NPM_TARBALL_REVISION;
use crate::s3::Buckets;
use crate::s3::CACHE_CONTROL_IMMUTABLE;
use crate::s3::S3Error;
use crate::s3::S3UploadOptions;
use crate::s3::UploadTaskBody;
use crate::s3_paths::file_path;
use crate::s3_paths::npm_tarball_path;
use crate::s3_paths::package_metadata;
use crate::s3_paths::version_metadata;
use crate::util::LicenseStore;

const MAX_FILE_SIZE: u64 = 20 * 1024 * 1024; // 20 MB
const MAX_TOTAL_FILE_SIZE: u64 = 20 * 1024 * 1024; // 20 MB
const HIGH_MAX_FILE_SIZE: u64 = 20 * 1024 * 1024; // 40 MB
const HIGH_MAX_TOTAL_FILE_SIZE: u64 = 20 * 1024 * 1024; // 40 MB
const MAX_CONCURRENT_UPLOADS: usize = 64;

static MEDIA_INFER: OnceLock<infer::Infer> = OnceLock::new();

/// Represents a resolved dependency with information about where it was resolved from.
#[derive(Debug, Clone, Eq, PartialEq, Hash)]
pub struct ResolvedDependency {
  pub kind: DependencyKind,
  pub req: PackageReqReference,
  /// If Some, the dependency was resolved from a fallback/external registry at this URL.
  /// If None, the dependency was resolved from the local registry.
  pub registry_url: Option<String>,
}

pub struct ProcessTarballOutput {
  pub file_infos: Vec<FileInfo>,
  pub module_graph_2: HashMap<String, deno_graph::analysis::ModuleInfo>,
  pub exports: ExportsMap,
  pub dependencies: HashSet<ResolvedDependency>,
  pub npm_tarball_info: NpmTarballInfo,
  pub readme_path: Option<PackagePath>,
  pub meta: PackageVersionMeta,
  pub doc_search_json: serde_json::Value,
  pub license: String,
}

pub struct NpmTarballInfo {
  /// The hex encoded sha1 hash of the gzipped tarball.
  pub sha1: String,
  /// The base64 encoded sha512 hash of the gzipped tarball.
  pub sha512: String,
  /// The size of the tarball in bytes.
  pub size: u64,
}

/// Upper bound on a single request to the fallback registry. Without it a
/// degraded fallback would stall the whole dependency resolution — and with it
/// the publishing task — for as long as it keeps the connection open.
pub const FALLBACK_REQUEST_TIMEOUT: std::time::Duration =
  std::time::Duration::from_secs(30);

/// Look up `req` in the fallback registry, returning the version it resolves to
/// there, or `None` if the registry has no matching non-yanked version that
/// exports `sub_path`. Errors are transport/protocol failures talking to the
/// fallback, which are retryable — see [`PublishError::user_error_code`].
async fn resolve_from_fallback(
  fallback_url: &Url,
  scope: &ScopeName,
  package: &PackageName,
  version_req: &VersionReq,
  sub_path: Option<&str>,
) -> Result<Option<Version>, PublishError> {
  let client = crate::util::shared_http_client();

  let meta_url = fallback_url
    .join(&package_metadata(scope, package))
    .map_err(|e| {
      PublishError::UnexpectedError(format!("Invalid fallback URL: {}", e))
    })?;

  let response = client
    .get(meta_url.clone())
    .timeout(FALLBACK_REQUEST_TIMEOUT)
    .send()
    .await
    .map_err(|e| PublishError::FallbackRegistryError {
      url: meta_url.to_string(),
      error: FallbackRegistryError::FetchPackageMetadata(e),
    })?;

  if !response.status().is_success() {
    if response.status() == StatusCode::NOT_FOUND {
      return Ok(None);
    } else {
      return Err(PublishError::FallbackRegistryError {
        url: meta_url.to_string(),
        error: FallbackRegistryError::PackageMetadataStatus(response.status()),
      });
    }
  }

  let package_meta: PackageMetadata =
    response
      .json()
      .await
      .map_err(|e| PublishError::FallbackRegistryError {
        url: meta_url.to_string(),
        error: FallbackRegistryError::ParsePackageMetadata(e),
      })?;

  let mut matching_versions: Vec<Version> = package_meta
    .versions
    .iter()
    .filter(|(_, v)| !v.yanked)
    .filter(|(v, _)| version_req.matches(&v.0))
    .map(|(v, _)| v.clone())
    .collect();

  matching_versions.sort_by(|a, b| b.0.cmp(&a.0));

  let exports_key = match sub_path {
    Some(path) if !path.is_empty() => format!("./{}", path),
    _ => ".".to_owned(),
  };

  for version in matching_versions {
    let version_meta_url = fallback_url
      .join(&version_metadata(scope, package, &version))
      .map_err(|e| {
        PublishError::UnexpectedError(format!("Invalid fallback URL: {}", e))
      })?;

    let response = client
      .get(version_meta_url.clone())
      .timeout(FALLBACK_REQUEST_TIMEOUT)
      .send()
      .await
      .map_err(|e| PublishError::FallbackRegistryError {
        url: version_meta_url.to_string(),
        error: FallbackRegistryError::FetchVersionMetadata(e),
      })?;

    if !response.status().is_success() {
      if response.status() == StatusCode::NOT_FOUND {
        continue;
      } else {
        return Err(PublishError::FallbackRegistryError {
          url: version_meta_url.to_string(),
          error: FallbackRegistryError::VersionMetadataStatus(
            response.status(),
          ),
        });
      }
    }

    let version_meta: VersionMetadata = response.json().await.map_err(|e| {
      PublishError::FallbackRegistryError {
        url: version_meta_url.to_string(),
        error: FallbackRegistryError::ParseVersionMetadata(e),
      }
    })?;

    if version_meta.exports.contains_key(&exports_key) {
      return Ok(Some(version));
    }
  }

  Ok(None)
}

static SUPPORTED_LICENSE_FILE_NAMES: [&str; 12] = [
  "/LICENSE",
  "/LICENSE.md",
  "/LICENSE.txt",
  "/LICENCE",
  "/LICENCE.md",
  "/LICENCE.txt",
  "/COPYING",
  "/COPYING.md",
  "/COPYING.txt",
  "/COPYING.LESSER",
  "/COPYING.LESSER.md",
  "/COPYING.LESSER.txt",
];

#[instrument(
  name = "process_tarball",
  skip(
    buckets,
    license_store,
    registry_url,
    fallback_registry_url,
    publishing_task
  ),
  err
)]
pub async fn process_tarball(
  db: &Database,
  buckets: &Buckets,
  license_store: &LicenseStore,
  registry_url: Url,
  fallback_registry_url: Option<Url>,
  publishing_task: &PublishingTask,
) -> Result<ProcessTarballOutput, PublishError> {
  let tarball_path = bucket_tarball_path(publishing_task.id);
  let stream = buckets
    .publishing_bucket
    .bucket
    .download_stream(&tarball_path, None)
    .await
    .map_err(PublishError::S3DownloadError)?
    .ok_or(PublishError::MissingTarball)?
    .map_err(io::Error::other);

  let async_read = stream.into_async_read();
  let decompressed =
    async_compression::futures::bufread::GzipDecoder::new(async_read);
  let mut tar = async_tar::Archive::new(decompressed)
    .entries()
    .map_err(from_tarball_io_error)?;

  let mut files = HashMap::new();
  let mut case_insensitive_paths = HashSet::<CaseInsensitivePackagePath>::new();
  let mut file_infos = Vec::new();
  let mut total_file_size = 0;

  // TODO: make these configurable through quota fields on the package
  let max_file_size = if *publishing_task.package_scope == "llamaindex"
    && *publishing_task.package_name == "core"
  {
    HIGH_MAX_FILE_SIZE
  } else {
    MAX_FILE_SIZE
  };
  let max_total_file_size = if *publishing_task.package_scope == "llamaindex"
    && *publishing_task.package_name == "core"
  {
    HIGH_MAX_TOTAL_FILE_SIZE
  } else {
    MAX_TOTAL_FILE_SIZE
  };

  while let Some(res) = tar.next().await {
    let mut entry = res.map_err(from_tarball_io_error)?;

    let header = entry.header();
    let path = String::from_utf8_lossy(&entry.path_bytes()).into_owned();
    let path = if path.starts_with("./") {
      path[1..].to_string()
    } else if !path.starts_with('/') {
      format!("/{}", path)
    } else {
      path
    };

    match header.entry_type() {
      EntryType::Regular => {}
      EntryType::Directory => continue,
      EntryType::Link | EntryType::Symlink => {
        return Err(PublishError::LinkInTarball { path });
      }
      _ => {
        return Err(PublishError::InvalidEntryType { path });
      }
    }

    let path = PackagePath::new(path.clone())
      .map_err(|error| PublishError::InvalidPath { path, error })?;

    if path.starts_with("/.git/") {
      return Err(PublishError::InvalidGitPath {
        path: path.to_string(),
      });
    }

    let size = header.size().map_err(from_tarball_io_error)?;
    if size > max_file_size {
      return Err(PublishError::FileTooLarge {
        path,
        max_size: max_file_size,
        size,
      });
    }
    total_file_size += size;

    if total_file_size > max_total_file_size {
      return Err(PublishError::PackageTooLarge {
        path,
        max_size: max_total_file_size,
        size: total_file_size,
      });
    }

    let mut bytes = Vec::new();
    entry
      .read_to_end(&mut bytes)
      .await
      .map_err(from_tarball_io_error)?;

    // sha256 hash the bytes
    let hash = sha2::Sha256::digest(&bytes);
    let hash = format!("sha256-{:x}", hash);

    // check for case-insensitive duplicate paths
    let case_insensitive_path = path.case_insensitive();
    if let Some(existing) = case_insensitive_paths.get(&case_insensitive_path) {
      return Err(PublishError::CaseInsensitiveDuplicatePath {
        a: path.clone(),
        b: existing.clone().into_inner().into_owned(),
      });
    }
    case_insensitive_paths.insert(case_insensitive_path.to_owned());

    if files.insert(path.clone(), bytes).is_some() {
      unreachable!("duplicate path: {:?}", path);
    }

    let file_info = FileInfo { path, hash, size };
    file_infos.push(file_info);
  }

  let config_file_bytes =
    files.get(&publishing_task.config_file).ok_or_else(|| {
      PublishError::MissingConfigFile(Box::new(
        publishing_task.config_file.clone(),
      ))
    })?;
  let config_file_str =
    std::str::from_utf8(config_file_bytes).map_err(|e| {
      PublishError::InvalidConfigFile {
        path: Box::new(publishing_task.config_file.clone()),
        error: e.into(),
      }
    })?;
  let config_file_value: serde_json::Value =
    jsonc_parser::parse_to_serde_value(
      config_file_str,
      &ParseOptions::default(),
    )
    .map_err(|e| PublishError::InvalidConfigFile {
      path: Box::new(publishing_task.config_file.clone()),
      error: e.into(),
    })?
    .ok_or(PublishError::InvalidConfigFile {
      path: Box::new(publishing_task.config_file.clone()),
      error: anyhow::anyhow!("config file must not be empty"),
    })?;
  let config_file: ConfigFile = serde_json::from_value(config_file_value)
    .map_err(|e| PublishError::InvalidConfigFile {
      path: Box::new(publishing_task.config_file.clone()),
      error: e.into(),
    })?;

  let publishing_task_scoped_package_name = ScopedPackageName {
    scope: publishing_task.package_scope.clone(),
    package: publishing_task.package_name.clone(),
  };
  if config_file.name != publishing_task_scoped_package_name {
    return Err(PublishError::ConfigFileNameMismatch {
      path: Box::new(publishing_task.config_file.clone()),
      deno_json_name: config_file.name,
      publish_task_name: publishing_task_scoped_package_name,
    });
  }
  if let Some(config_file_version) = config_file.version
    && config_file_version != publishing_task.package_version
  {
    return Err(PublishError::ConfigFileVersionMismatch {
      path: Box::new(publishing_task.config_file.clone()),
      deno_json_version: Box::new(config_file_version),
      publish_task_version: Box::new(publishing_task.package_version.clone()),
    });
  }

  let exports =
    exports_map_from_json(config_file.exports).map_err(|invalid_exports| {
      PublishError::ConfigFileExportsInvalid {
        path: Box::new(publishing_task.config_file.clone()),
        invalid_exports,
      }
    })?;

  if exports.is_empty() {
    return Err(PublishError::ConfigFileExportsInvalid {
      path: Box::new(publishing_task.config_file.clone()),
      invalid_exports: "exports config must have at least one entry"
        .to_string(),
    });
  }

  let license = if let Some(license) = config_file.license {
    if !license_store.is_recognized(&license) {
      return Err(PublishError::InvalidLicense);
    } else {
      license
    }
  } else {
    let mut license = None;
    for license_file_name in SUPPORTED_LICENSE_FILE_NAMES {
      if let Some(license_file) =
        files.get(&PackagePath::new(license_file_name.to_string()).unwrap())
      {
        let license_content = String::from_utf8_lossy(license_file);
        let analyzed = license_store
          .0
          .analyze(&askalono::TextData::new(license_content.as_ref()));
        if analyzed.score > 0.8 {
          license = Some(analyzed.name.to_string());
        } else {
          return Err(PublishError::InvalidLicense);
        }

        break;
      }
    }

    license.ok_or_else(|| PublishError::MissingLicense)?
  };

  let span = Span::current();
  let scope = publishing_task.package_scope.clone();
  let package = publishing_task.package_name.clone();
  let version = publishing_task.package_version.clone();
  let config_file = publishing_task.config_file.clone();
  let analysis_data = PackageAnalysisData { exports, files };
  let PackageAnalysisOutput {
    data: PackageAnalysisData { exports, files },
    module_graph_2,
    doc_nodes_bytes,
    doc_search_json,
    dependencies,
    npm_tarball,
    readme_path,
    meta,
  } = tokio::task::spawn_blocking(|| {
    analyze_package(
      span,
      registry_url,
      scope,
      package,
      version,
      config_file,
      analysis_data,
    )
  })
  .await
  .map_err(|e| PublishError::UnexpectedError(format!("{:?}", e)))??;

  let mut resolved_dependencies: HashSet<ResolvedDependency> = HashSet::new();

  for (kind, req) in dependencies.iter() {
    if kind == &DependencyKind::Jsr {
      let package_scope = ScopedPackageName::new(req.req.name.to_string())
        .map_err(|e| {
          PublishError::InvalidJsrScopedPackageName(req.req.name.clone(), e)
        })?;

      let mut versions = db
        .list_package_versions_for_resolution(
          &package_scope.scope,
          &package_scope.package,
        )
        .await?;
      versions.sort_by(|a, b| b.version.cmp(&a.version));

      // A package this registry hosts (in any version) is always served
      // locally: its meta.json is present in the bucket, and the lb only
      // consults the fallback on a bucket miss. So resolution must succeed
      // against the local versions alone — validating such a dependency
      // against the fallback would accept one that no consumer can install.
      if !versions.is_empty() {
        let mut found = false;

        for version in versions.iter().rev() {
          if req.req.version_req.matches(&version.version.0) {
            let exports_key = if let Some(sub_path) = &req.sub_path {
              if sub_path.is_empty() {
                ".".to_owned()
              } else {
                format!("./{}", sub_path)
              }
            } else {
              ".".to_owned()
            };

            if !version.exports.contains_key(&exports_key) {
              return Err(PublishError::InvalidJsrDependencySubPath {
                req: Box::new(req.clone()),
                resolved_version: version.version.clone(),
                exports_key,
              });
            }

            found = true;
            break;
          }
        }

        if !found {
          return Err(PublishError::UnresolvableJsrDependency(req.req.clone()));
        }

        resolved_dependencies.insert(ResolvedDependency {
          kind: *kind,
          req: req.clone(),
          registry_url: None,
        });
        continue;
      }

      // Not in this registry — consult the fallback, if one is configured. The
      // `?` matters: a fallback that is unreachable or misbehaving must surface
      // as a retryable task error, not be silently folded into the fatal
      // `UnresolvableJsrDependency` below, which would tell the publisher their
      // dependency does not exist when we simply failed to look it up.
      let fallback_resolution = match &fallback_registry_url {
        Some(fallback_url) => resolve_from_fallback(
          fallback_url,
          &package_scope.scope,
          &package_scope.package,
          &req.req.version_req,
          req.sub_path.as_deref(),
        )
        .await?
        .map(|_resolved_version| fallback_url.to_string()),
        None => None,
      };

      // Only the fallback's identity is recorded, not the version it resolved
      // to right now: like a locally-resolved dependency, the row stores the
      // constraint and consumers re-resolve it against the fallback at install
      // time.
      if let Some(fallback_url) = fallback_resolution {
        resolved_dependencies.insert(ResolvedDependency {
          kind: *kind,
          req: req.clone(),
          registry_url: Some(fallback_url),
        });
      } else {
        return Err(PublishError::UnresolvableJsrDependency(req.req.clone()));
      }
    } else {
      // Npm dependencies aren't resolved at publish time, but a `@jsr/`-mapped
      // dependency on a package this registry doesn't host is served by the
      // npm fallback at install time — record the fallback so the frontend
      // links there instead of to a local package page that 404s.
      let mut registry_url = None;
      if let Some(fallback_url) = &fallback_registry_url
        && let Some((scope, package)) = req
          .req
          .name
          .strip_prefix("@jsr/")
          .and_then(|rest| rest.split_once("__"))
        && let (Ok(scope), Ok(package)) =
          (ScopeName::try_from(scope), PackageName::try_from(package))
        && db.get_package(&scope, &package).await?.is_none()
      {
        registry_url = Some(fallback_url.to_string());
      }

      resolved_dependencies.insert(ResolvedDependency {
        kind: *kind,
        req: req.clone(),
        registry_url,
      });
    }
  }

  // TO ENSURE CONSISTENCY OF FILES IN S3, ALL ERRORS RETURNED AFTER THIS POINT MUST BE RETRYABLE

  buckets
    .docs_bucket
    .upload(
      crate::s3_paths::docs_v2_path(
        &publishing_task.package_scope,
        &publishing_task.package_name,
        &publishing_task.package_version,
      )
      .into(),
      crate::s3::UploadTaskBody::Bytes(doc_nodes_bytes),
      S3UploadOptions {
        content_type: Some("application/x-msgpack".into()),
        cache_control: Some(CACHE_CONTROL_IMMUTABLE.into()),
        gzip_encoded: true,
      },
    )
    .await
    .map_err(PublishError::S3UploadError)?;

  let npm_tarball_info = NpmTarballInfo {
    sha1: npm_tarball.sha1,
    sha512: npm_tarball.sha512,
    size: npm_tarball.tarball.len() as u64,
  };

  let npm_tarball_path = npm_tarball_path(
    &publishing_task.package_scope,
    &publishing_task.package_name,
    &publishing_task.package_version,
    NPM_TARBALL_REVISION,
  );
  buckets
    .npm_bucket
    .upload(
      npm_tarball_path.into(),
      crate::s3::UploadTaskBody::Bytes(Bytes::from(npm_tarball.tarball)),
      S3UploadOptions {
        content_type: Some("application/octet-stream".into()),
        cache_control: Some(CACHE_CONTROL_IMMUTABLE.into()),
        gzip_encoded: false,
      },
    )
    .await
    .map_err(PublishError::S3UploadError)?;

  let mut uploads = futures::stream::iter(files)
    .map(|(path, data)| {
      let bytes = Bytes::from(data);
      let media_type = MediaType::from_str(&path);
      let maybe_content_type = media_type
        .as_content_type()
        .map(|str| str.to_string())
        .or_else(|| {
          MEDIA_INFER
            .get_or_init(|| {
              let mut media_infer = infer::Infer::new();
              media_infer.add("image/svg+xml", "svg", |content_bytes| {
                (content_bytes.starts_with(b"<svg")
                  || content_bytes.starts_with(b"<?xml"))
                  && content_bytes.ends_with(b"</svg>")
              });
              media_infer
            })
            .get(&bytes)
            .map(|mimetype| mimetype.mime_type().to_string())
        });
      (path, bytes, maybe_content_type)
    })
    .map(|(path, bytes, maybe_content_type)| {
      let s3_path = file_path(
        &publishing_task.package_scope,
        &publishing_task.package_name,
        &publishing_task.package_version,
        &path,
      );

      async move {
        buckets
          .modules_bucket
          .upload(
            s3_path.into(),
            UploadTaskBody::Bytes(bytes),
            S3UploadOptions {
              content_type: maybe_content_type.map(Into::into),
              cache_control: Some(CACHE_CONTROL_IMMUTABLE.into()),
              gzip_encoded: false,
            },
          )
          .await
          .map_err(PublishError::S3UploadError)
      }
    })
    .buffer_unordered(MAX_CONCURRENT_UPLOADS);

  while let Some(res) = uploads.next().await {
    res?;
  }

  drop(uploads);

  Ok(ProcessTarballOutput {
    file_infos,
    module_graph_2,
    exports,
    dependencies: resolved_dependencies,
    npm_tarball_info,
    readme_path,
    meta,
    doc_search_json,
    license,
  })
}

pub fn bucket_tarball_path(id: Uuid) -> String {
  format!("publishing_tasks/{}.tar.gz", id)
}

#[derive(Debug, Error)]
pub enum FallbackRegistryError {
  #[error("failed to fetch package metadata: {0}")]
  FetchPackageMetadata(reqwest::Error),
  #[error("unexpected status code fetching package metadata: {0}")]
  PackageMetadataStatus(StatusCode),
  #[error("failed to parse package metadata: {0}")]
  ParsePackageMetadata(reqwest::Error),
  #[error("failed to fetch version metadata: {0}")]
  FetchVersionMetadata(reqwest::Error),
  #[error("unexpected status code fetching version metadata: {0}")]
  VersionMetadataStatus(StatusCode),
  #[error("failed to parse version metadata: {0}")]
  ParseVersionMetadata(reqwest::Error),
}

#[derive(Debug, Error)]
pub enum PublishError {
  #[error("s3 download error: {0}")]
  S3DownloadError(S3Error),

  #[error("missing tarball")]
  MissingTarball,

  #[error("s3 upload error: {0}")]
  S3UploadError(S3Error),

  #[error("invalid tarball: {0}")]
  InvalidTarball(io::Error),

  #[error("database error")]
  DatabaseError(#[from] sqlx::Error),

  #[error(
    "entry at '{path}' is a link, only regular files and directories are allowed"
  )]
  LinkInTarball { path: String },

  #[error("entry at '{path}' is not a regular file or directory")]
  InvalidEntryType { path: String },

  #[error("path '{path}' is invalid: {error}")]
  InvalidPath {
    path: String,
    error: PackagePathValidationError,
  },

  #[error("path '{path}' is invalid: .git files are not allowed")]
  InvalidGitPath { path: String },

  #[error(
    "invalid external import to '{specifier}', only 'jsr:', 'npm:', 'data:', 'bun:', and 'node:' imports are allowed ({info})"
  )]
  InvalidExternalImport { specifier: String, info: String },

  #[error("modifying global types is not allowed {specifier}:{line}:{column}")]
  GlobalTypeAugmentation {
    specifier: String,
    line: usize,
    column: usize,
  },

  #[error("CommonJS is not allowed {specifier}:{line}:{column}")]
  CommonJs {
    specifier: String,
    line: usize,
    column: usize,
  },

  #[error(
    "triple slash directives that modify globals (for example, '/// <reference no-default-lib=\"true\" />' or '/// <reference lib=\"dom\" />') are not allowed. Instead instruct the user of your package to specify these directives. {specifier}:{line}:{column}"
  )]
  BannedTripleSlashDirectives {
    specifier: String,
    line: usize,
    column: usize,
  },

  #[error(
    "import assertions are not allowed, use import attributes instead (replace 'assert' with 'with') {specifier}:{line}:{column}"
  )]
  BannedImportAssertion {
    specifier: String,
    line: usize,
    column: usize,
  },

  #[error(
    "file at path '{path}' too large, max size is {max_size}, got {size}"
  )]
  FileTooLarge {
    path: PackagePath,
    max_size: u64,
    size: u64,
  },

  #[error(
    "package too large as limit has been exceeded by '{path}', max size is {max_size}, got {size}"
  )]
  PackageTooLarge {
    path: PackagePath,
    max_size: u64,
    size: u64,
  },

  #[error("case-insensitive duplicate path '{a}' and '{b}'")]
  CaseInsensitiveDuplicatePath { a: PackagePath, b: PackagePath },

  #[error("missing config file '{0}', is it perhaps excluded from publishing?")]
  MissingConfigFile(Box<PackagePath>),

  #[error("invalid config file '{path}': {error}")]
  InvalidConfigFile {
    path: Box<PackagePath>,
    error: anyhow::Error,
  },

  #[error(
    "package name specified during publish does not match name in config file '{path}', expected {publish_task_name}, got {deno_json_name}"
  )]
  ConfigFileNameMismatch {
    path: Box<PackagePath>,
    deno_json_name: ScopedPackageName,
    publish_task_name: ScopedPackageName,
  },
  #[error(
    "version specified during publish does not match version in config file '{path}', expected {publish_task_version}, got {deno_json_version}"
  )]
  ConfigFileVersionMismatch {
    path: Box<PackagePath>,
    deno_json_version: Box<Version>,
    publish_task_version: Box<Version>,
  },
  #[error("invalid 'exports' field in config file '{path}': {invalid_exports}")]
  ConfigFileExportsInvalid {
    path: Box<PackagePath>,
    invalid_exports: String,
  },

  #[error("failed to build module graph: {}", .0.to_string_with_range())]
  GraphError(Box<ModuleGraphError>),

  #[error("failed to generate documentation: {0:?}")]
  DocError(anyhow::Error),

  #[error("failed to generate NPM tarball: {0}")]
  NpmTarballError(anyhow::Error),

  #[error("invalid 'jsr:' specifier: {0}")]
  InvalidJsrSpecifier(PackageReqReferenceParseError),

  #[error("invalid 'npm:' specifier: {0}")]
  InvalidNpmSpecifier(PackageReqReferenceParseError),

  #[error("specifier '{0}' is missing a version constraint")]
  JsrMissingConstraint(JsrPackageReqReference),

  #[error("specifier '{0}' is missing a version constraint")]
  NpmMissingConstraint(NpmPackageReqReference),

  #[error("invalid scoped package name in 'jsr:' specifier '{0}': {1}")]
  InvalidJsrScopedPackageName(
    deno_semver::StackString,
    ScopedPackageNameValidateError,
  ),

  #[error("unexpected error: {0}")]
  UnexpectedError(String),

  #[error("failed to resolve from fallback registry '{url}': {error}")]
  FallbackRegistryError {
    url: String,
    error: FallbackRegistryError,
  },

  #[error(
    "unresolvable 'jsr:' dependency: '{0}', no published version matches the constraint"
  )]
  UnresolvableJsrDependency(PackageReq),

  #[error(
    "invalid 'jsr:' dependency subpath: '{req}', resolved to {resolved_version}, has no export '{exports_key}'"
  )]
  InvalidJsrDependencySubPath {
    req: Box<PackageReqReference>,
    resolved_version: Version,
    exports_key: String,
  },

  #[error(
    "No license was specified. Either provide a LICENSE file or specify the \"license\" field in your configuration file."
  )]
  MissingLicense,

  #[error(
    "The license specified in the \"license\" field of your configuration file, or in the LICENSE file was not recognized."
  )]
  InvalidLicense,
}

impl PublishError {
  /// User errors are not retryable and should be propagated to the user. All
  /// other errors are retryable, and displayed as internal errors to users.
  pub fn user_error_code(&self) -> Option<&'static str> {
    match self {
      PublishError::S3DownloadError(_) => None,
      PublishError::S3UploadError(_) => None,
      PublishError::MissingTarball => None,
      PublishError::DatabaseError(_) => None,
      PublishError::UnexpectedError(_) => None,
      PublishError::InvalidTarball(_) => Some("invalidTarball"),
      PublishError::LinkInTarball { .. } => Some("linkInTarball"),
      PublishError::InvalidEntryType { .. } => Some("invalidEntryType"),
      PublishError::InvalidPath { .. } => Some("invalidPath"),
      PublishError::InvalidGitPath { .. } => Some("invalidGitPath"),
      PublishError::GlobalTypeAugmentation { .. } => {
        Some("globalTypeAugmentation")
      }
      PublishError::CommonJs { .. } => Some("commonJs"),
      PublishError::BannedTripleSlashDirectives { .. } => {
        Some("bannedTripleSlashDirectives")
      }
      PublishError::BannedImportAssertion { .. } => {
        Some("bannedImportAssertion")
      }
      PublishError::InvalidExternalImport { .. } => {
        Some("invalidExternalImport")
      }
      PublishError::FileTooLarge { .. } => Some("fileTooLarge"),
      PublishError::PackageTooLarge { .. } => Some("packageTooLarge"),
      PublishError::CaseInsensitiveDuplicatePath { .. } => {
        Some("caseInsensitiveDuplicatePath")
      }
      PublishError::MissingConfigFile(_) => Some("missingConfigFile"),
      PublishError::InvalidConfigFile { .. } => Some("invalidConfigFile"),
      PublishError::ConfigFileNameMismatch { .. } => {
        Some("configFileNameMismatch")
      }
      PublishError::ConfigFileVersionMismatch { .. } => {
        Some("configFileVersionMismatch")
      }
      PublishError::ConfigFileExportsInvalid { .. } => {
        Some("configFileExportsInvalid")
      }
      PublishError::GraphError(_) => Some("graphError"),
      PublishError::DocError(_) => Some("docError"),
      PublishError::NpmTarballError(_) => Some("npmTarballError"),
      PublishError::InvalidJsrSpecifier(_) => Some("invalidJsrSpecifier"),
      PublishError::InvalidNpmSpecifier(_) => Some("invalidNpmSpecifier"),
      PublishError::JsrMissingConstraint(_) => Some("missingConstraint"),
      PublishError::NpmMissingConstraint(_) => Some("missingConstraint"),
      PublishError::InvalidJsrScopedPackageName(_, _) => {
        Some("invalidJsrScopedPackageName")
      }
      PublishError::UnresolvableJsrDependency(_) => {
        Some("unresolvableJsrDependency")
      }
      PublishError::InvalidJsrDependencySubPath { .. } => {
        Some("invalidJsrDependencySubPath")
      }
      PublishError::MissingLicense => Some("missingLicense"),
      PublishError::InvalidLicense => Some("invalidLicense"),
      // Not the publisher's fault: the fallback registry is a piece of this
      // instance's infrastructure. Failing a publish outright because it was
      // briefly unreachable would reject packages that are perfectly valid, so
      // this stays retryable like the other infrastructure errors above.
      PublishError::FallbackRegistryError { .. } => None,
    }
  }
}

fn from_tarball_io_error(err: io::Error) -> PublishError {
  match err.downcast::<s3::error::S3Error>() {
    Ok(err) => PublishError::S3DownloadError(S3Error::S3(err)),
    Err(err) => PublishError::InvalidTarball(err),
  }
}

pub struct FileInfo {
  pub path: PackagePath,
  pub size: u64,
  pub hash: String, // todo, use a wrapper struct/enum
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ConfigFile {
  pub name: ScopedPackageName,
  pub version: Option<Version>,
  pub license: Option<String>,
  pub exports: Option<serde_json::Value>,
}

pub fn exports_map_from_json(
  exports: Option<serde_json::Value>,
) -> Result<ExportsMap, String> {
  fn has_extension(value: &str) -> bool {
    let search_text = value.rsplit('/').next().unwrap();
    search_text.contains('.')
  }

  fn validate_key(key: &str) -> Result<(), String> {
    if key == "." {
      return Ok(());
    }
    if !key.starts_with("./") {
      let suggestion = if key.starts_with('/') {
        format!(".{}", key)
      } else {
        format!("./{}", key)
      };
      return Err(format!(
        "the key '{key}' must start with a ./, did you mean '{suggestion}'?"
      ));
    }
    if key.ends_with('/') {
      let suggestion = key.trim_end_matches('/');
      return Err(format!(
        "the key '{key}' must not end with '/', did you mean '{suggestion}'?",
      ));
    }
    // ban anything that is not [a-zA-Z0-9_-./]
    if !key.chars().all(|c| {
      matches!(c, 'a'..='z' | 'A'..='Z' | '0'..='9' | '-' | '_' | '/' | '.')
    }) {
      return Err(format!(
        "the key '{key}' contains invalid characters, only [a-z][A-Z][0-9]-_/. are allowed",
      ));
    }
    // ban parts consisting of only dots, and empty parts (e.g. `./foo//bar`)
    for part in key.split('/').skip(1) {
      if part.is_empty() || part.chars().all(|c| c == '.') {
        return Err(format!(
          "the key '{key}' must not contain double slashes (//) or parts entirely of dots (.).",
        ));
      }
    }
    Ok(())
  }

  fn validate_value(key: &str, value: &str) -> Result<(), String> {
    if value.is_empty() {
      return Err(format!(
        "the path for {key} must be a non-empty relative path"
      ));
    }
    if !value.starts_with("./") {
      return Err(format!(
        "the path '{value}' for {key} could not be resolved as a relative path from the config file, did you mean './{value}'?"
      ));
    }
    if value.ends_with('/') || !has_extension(value) {
      return Err(format!(
        "the path '{value}' for {key} must not end in / and must have a file extension"
      ));
    }
    Ok(())
  }

  let exports = match exports {
    None => {
      return Ok(ExportsMap::new(IndexMap::new()));
    }
    Some(serde_json::Value::String(val)) => {
      validate_value("the root export", &val)?;
      return Ok(ExportsMap::new(IndexMap::from([(".".to_string(), val)])));
    }
    Some(serde_json::Value::Object(map)) => map,
    Some(serde_json::Value::Array(_))
    | Some(serde_json::Value::Bool(_))
    | Some(serde_json::Value::Number(_))
    | Some(serde_json::Value::Null) => {
      return Err("'exports' field must be a string or an object".to_string());
    }
  };

  let mut result = IndexMap::new();

  for (key, value) in exports {
    validate_key(&key)?;
    let value = match value {
      serde_json::Value::String(value) => value,
      _ => {
        return Err(format!(
          "export '{key}' must be a string, invalid value: '{value}'",
        ));
      }
    };
    validate_value(&format!("export '{key}'"), &value)?;
    result.insert(key, value);
  }

  Ok(ExportsMap::new(result))
}

#[cfg(test)]
mod tests {
  macro_rules! exports_map_from_json_error {
    ($name:ident, $json:tt, $expected:expr) => {
      #[test]
      fn $name() {
        let json = serde_json::json!($json);
        assert_eq!(
          super::exports_map_from_json(Some(json)).unwrap_err(),
          $expected
        );
      }
    };
  }

  exports_map_from_json_error!(
    empty,
    null,
    "'exports' field must be a string or an object"
  );
  exports_map_from_json_error!(
    array,
    [],
    "'exports' field must be a string or an object"
  );
  exports_map_from_json_error!(
    bool,
    true,
    "'exports' field must be a string or an object"
  );
  exports_map_from_json_error!(
    number,
    1,
    "'exports' field must be a string or an object"
  );

  exports_map_from_json_error!(
    invalid_root_path_1,
    "",
    "the path for the root export must be a non-empty relative path"
  );
  exports_map_from_json_error!(
    invalid_root_path_2,
    "foo",
    "the path 'foo' for the root export could not be resolved as a relative path from the config file, did you mean './foo'?"
  );
  exports_map_from_json_error!(
    invalid_root_path_3,
    "./",
    "the path './' for the root export must not end in / and must have a file extension"
  );

  exports_map_from_json_error!(
    invalid_key_1,
    { "foo": "./bar" },
    "the key 'foo' must start with a ./, did you mean './foo'?"
  );
  exports_map_from_json_error!(
    invalid_key_2,
    { "./foo/": "./bar" },
    "the key './foo/' must not end with '/', did you mean './foo'?"
  );
  exports_map_from_json_error!(
    invalid_key_3,
    { "./foo/~/bar": "./bar" },
    "the key './foo/~/bar' contains invalid characters, only [a-z][A-Z][0-9]-_/. are allowed"
  );

  exports_map_from_json_error!(
    invalid_value_1,
    { "./foo": 1 },
    "export './foo' must be a string, invalid value: '1'"
  );
}
