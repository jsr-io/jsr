// Copyright 2024 the JSR authors. All rights reserved. MIT license.
use std::collections::HashMap;
use std::collections::HashSet;
use std::io;
use std::sync::OnceLock;

use async_tar::EntryType;
use bytes::Bytes;
use deno_ast::MediaType;
use deno_graph::ModuleGraphError;
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
use serde::Deserialize;
use serde::Serialize;
use sha2::Digest;
use thiserror::Error;
use tracing::instrument;
use tracing::Span;
use url::Url;
use uuid::Uuid;

use crate::analysis::analyze_package;
use crate::analysis::PackageAnalysisData;
use crate::analysis::PackageAnalysisOutput;
use crate::buckets::Buckets;
use crate::buckets::UploadTaskBody;
use crate::db::Database;
use crate::db::ExportsMap;
use crate::db::PublishingTask;
use crate::db::{DependencyKind, PackageVersionMeta};
use crate::gcp::GcsError;
use crate::gcp::GcsUploadOptions;
use crate::gcp::CACHE_CONTROL_IMMUTABLE;
use crate::gcs_paths::docs_v1_path;
use crate::gcs_paths::file_path;
use crate::gcs_paths::npm_tarball_path;
use crate::ids::CaseInsensitivePackagePath;
use crate::ids::PackagePath;
use crate::ids::PackagePathValidationError;
use crate::ids::ScopedPackageName;
use crate::ids::ScopedPackageNameValidateError;
use crate::ids::Version;
use crate::npm::NPM_TARBALL_REVISION;

const MAX_FILE_SIZE: u64 = 20 * 1024 * 1024; // 20 MB
const MAX_TOTAL_FILE_SIZE: u64 = 20 * 1024 * 1024; // 20 MB
const MAX_CONCURRENT_UPLOADS: usize = 1024;

static MEDIA_INFER: OnceLock<infer::Infer> = OnceLock::new();

pub struct ProcessTarballOutput {
  pub file_infos: Vec<FileInfo>,
  pub module_graph_2: HashMap<String, deno_graph::ModuleInfo>,
  pub exports: ExportsMap,
  pub dependencies: HashSet<(DependencyKind, PackageReqReference)>,
  pub npm_tarball_info: NpmTarballInfo,
  pub readme_path: Option<PackagePath>,
  pub meta: PackageVersionMeta,
}

pub struct NpmTarballInfo {
  /// The hex encoded sha1 hash of the gzipped tarball.
  pub sha1: String,
  /// The base64 encoded sha512 hash of the gzipped tarball.
  pub sha512: String,
  /// The size of the tarball in bytes.
  pub size: u64,
}

#[instrument(
  name = "process_tarball",
  skip(buckets, registry_url, publishing_task),
  err
)]
pub async fn process_tarball(
  db: &Database,
  buckets: &Buckets,
  registry_url: Url,
  publishing_task: &PublishingTask,
) -> Result<ProcessTarballOutput, PublishError> {
  let tarball_path = gcs_tarball_path(publishing_task.id);
  let stream = buckets
    .publishing_bucket
    .bucket
    .download_stream(&tarball_path, None)
    .await
    .map_err(PublishError::GcsDownloadError)?
    .ok_or(PublishError::MissingTarball)?
    .map_err(|e| io::Error::new(io::ErrorKind::Other, e));

  let async_read = stream.into_async_read();
  let mut tar = async_tar::Archive::new(async_read)
    .entries()
    .map_err(PublishError::UntarError)?;

  let mut files = HashMap::new();
  let mut case_insensitive_paths = HashSet::<CaseInsensitivePackagePath>::new();
  let mut file_infos = Vec::new();
  let mut total_file_size = 0;

  while let Some(res) = tar.next().await {
    let mut entry = res.map_err(PublishError::UntarError)?;

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

    let size = header.size().map_err(PublishError::UntarError)?;
    if size > MAX_FILE_SIZE {
      return Err(PublishError::FileTooLarge {
        path,
        max_size: MAX_FILE_SIZE,
        size,
      });
    }
    total_file_size += size;
    if total_file_size > MAX_TOTAL_FILE_SIZE {
      return Err(PublishError::PackageTooLarge {
        path,
        max_size: MAX_TOTAL_FILE_SIZE,
        size: total_file_size,
      });
    }

    let mut bytes = Vec::new();
    entry
      .read_to_end(&mut bytes)
      .await
      .map_err(PublishError::UntarError)?;

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
  if config_file.version != publishing_task.package_version {
    return Err(PublishError::ConfigFileVersionMismatch {
      path: Box::new(publishing_task.config_file.clone()),
      deno_json_version: Box::new(config_file.version),
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

  let span = Span::current();
  let scope = publishing_task.package_scope.clone();
  let package = publishing_task.package_name.clone();
  let version = publishing_task.package_version.clone();
  let config_file = publishing_task.config_file.clone();
  let analysis_data = PackageAnalysisData { exports, files };
  let PackageAnalysisOutput {
    data: PackageAnalysisData { exports, files },
    module_graph_2,
    doc_nodes,
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
  .unwrap()?;

  // ensure all of the JSR dependencies are resolvable
  for (kind, req) in dependencies.iter() {
    if kind == &DependencyKind::Jsr {
      let package_scope = ScopedPackageName::new(req.req.name.clone())
        .map_err(|e| {
          PublishError::InvalidJsrScopedPackageName(req.req.name.clone(), e)
        })?;

      let mut versions = db
        .list_package_versions(&package_scope.scope, &package_scope.package)
        .await?
        .into_iter()
        .map(|v| v.0)
        .collect::<Vec<_>>();
      versions.sort_by_cached_key(|v| v.version.clone());

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
    }
  }

  // TO ENSURE CONSISTENCY OF FILES IN GCS, ALL ERRORS RETURNED AFTER THIS POINT MUST BE RETRYABLE

  buckets
    .docs_bucket
    .upload(
      docs_v1_path(
        &publishing_task.package_scope,
        &publishing_task.package_name,
        &publishing_task.package_version,
      )
      .into(),
      UploadTaskBody::Bytes(
        serde_json::to_vec(&doc_nodes)
          .expect("failed to serialize doc_nodes")
          .into(),
      ),
      GcsUploadOptions {
        content_type: Some("application/json".into()),
        cache_control: Some(CACHE_CONTROL_IMMUTABLE.into()),
        gzip_encoded: false,
      },
    )
    .await
    .map_err(PublishError::GcsUploadError)?;

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
      UploadTaskBody::Bytes(Bytes::from(npm_tarball.tarball)),
      GcsUploadOptions {
        content_type: Some("application/octet-stream".into()),
        cache_control: Some(CACHE_CONTROL_IMMUTABLE.into()),
        gzip_encoded: false,
      },
    )
    .await
    .map_err(PublishError::GcsUploadError)?;

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
      let gcs_path = file_path(
        &publishing_task.package_scope,
        &publishing_task.package_name,
        &publishing_task.package_version,
        &path,
      );

      async move {
        buckets
          .modules_bucket
          .upload(
            gcs_path.into(),
            UploadTaskBody::Bytes(bytes),
            GcsUploadOptions {
              content_type: maybe_content_type.map(Into::into),
              cache_control: Some(CACHE_CONTROL_IMMUTABLE.into()),
              gzip_encoded: false,
            },
          )
          .await
          .map_err(PublishError::GcsUploadError)
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
    dependencies,
    npm_tarball_info,
    readme_path,
    meta,
  })
}

pub fn gcs_tarball_path(id: Uuid) -> String {
  format!("publishing_tasks/{}.tar.gz", id)
}

#[derive(Debug, Error)]
pub enum PublishError {
  #[error("gcs download error: {0}")]
  GcsDownloadError(GcsError),

  #[error("missing tarball")]
  MissingTarball,

  #[error("gcs upload error: {0}")]
  GcsUploadError(GcsError),

  #[error("untar error: {0}")]
  UntarError(io::Error),

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

  #[error("invalid external import to '{specifier}', only 'jsr:', 'npm:', 'data:' and 'node:' imports are allowed ({info})")]
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

  #[error("triple slash directives that modify globals (for example, '/// <reference no-default-lib=\"true\" />' or '/// <reference lib=\"dom\" />') are not allowed. Instead instruct the user of your package to specify these directives. {specifier}:{line}:{column}")]
  BannedTripleSlashDirectives {
    specifier: String,
    line: usize,
    column: usize,
  },

  #[error("import assertions are not allowed, use import attributes instead (replace 'assert' with 'with') {specifier}:{line}:{column}")]
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

  #[error(
    "missing config file '{0}', is it perhaps excluded from publishing?"
  )]
  MissingConfigFile(Box<PackagePath>),

  #[error("invalid config file '{path}': {error}")]
  InvalidConfigFile {
    path: Box<PackagePath>,
    error: anyhow::Error,
  },

  #[error("package name specified during publish does not match name in config file '{path}', expected {publish_task_name}, got {deno_json_name}")]
  ConfigFileNameMismatch {
    path: Box<PackagePath>,
    deno_json_name: ScopedPackageName,
    publish_task_name: ScopedPackageName,
  },
  #[error("version specified during publish does not match version in config file '{path}', expected {publish_task_version}, got {deno_json_version}")]
  ConfigFileVersionMismatch {
    path: Box<PackagePath>,
    deno_json_version: Box<Version>,
    publish_task_version: Box<Version>,
  },
  #[error(
    "invalid 'exports' field in config file '{path}': {invalid_exports}"
  )]
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
  InvalidJsrScopedPackageName(String, ScopedPackageNameValidateError),

  #[error("unresolvable 'jsr:' dependency: '{0}', no published version matches the constraint")]
  UnresolvableJsrDependency(PackageReq),

  #[error("invalid 'jsr:' dependency subpath: '{req}', resolved to {resolved_version}, has no export '{exports_key}'")]
  InvalidJsrDependencySubPath {
    req: Box<PackageReqReference>,
    resolved_version: Version,
    exports_key: String,
  },
}

impl PublishError {
  /// User errors are not retryable and should be propagated to the user. All
  /// other errors are retryable, and displayed as internal errors to users.
  pub fn user_error_code(&self) -> Option<&'static str> {
    match self {
      PublishError::GcsDownloadError(_) => None,
      PublishError::GcsUploadError(_) => None,
      PublishError::UntarError(_) => None,
      PublishError::MissingTarball => None,
      PublishError::DatabaseError(_) => None,
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
      PublishError::JsrMissingConstraint(_) => Some("jsrMissingConstraint"),
      PublishError::NpmMissingConstraint(_) => Some("npmMissingConstraint"),
      PublishError::InvalidJsrScopedPackageName(_, _) => {
        Some("invalidJsrScopedPackageName")
      }
      PublishError::UnresolvableJsrDependency(_) => {
        Some("unresolvableJsrDependency")
      }
      PublishError::InvalidJsrDependencySubPath { .. } => {
        Some("invalidJsrDependencySubPath")
      }
    }
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
  pub version: Version,
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
      return Err(format!("the path '{value}' for {key} could not be resolved as a relative path from the config file, did you mean './{value}'?"));
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
