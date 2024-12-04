// Copyright 2024 the JSR authors. All rights reserved. MIT license.
use std::borrow::Cow;
use std::collections::HashSet;
use std::io;
use std::sync::atomic::AtomicU64;
use std::sync::atomic::Ordering;
use std::sync::Arc;
use std::sync::Mutex;

use anyhow::Context;
use chrono::Utc;
use comrak::adapters::SyntaxHighlighterAdapter;
use futures::future::Either;
use futures::StreamExt;
use hyper::body::HttpBody;
use hyper::Body;
use hyper::Request;
use hyper::Response;
use hyper::StatusCode;
use routerify::prelude::RequestExt;
use routerify::Router;
use routerify_query::RequestQueryExt;
use sha2::Digest;
use tracing::error;
use tracing::field;
use tracing::instrument;
use tracing::Instrument;
use tracing::Span;
use url::Url;

use crate::auth::access_token;
use crate::auth::GithubOauth2Client;
use crate::buckets::Buckets;
use crate::buckets::UploadTaskBody;
use crate::db::CreatePackageResult;
use crate::db::CreatePublishingTaskResult;
use crate::db::Database;
use crate::db::NewGithubRepository;
use crate::db::NewPublishingTask;
use crate::db::Package;
use crate::db::RuntimeCompat;
use crate::db::User;
use crate::docs::DocNodesByUrl;
use crate::docs::DocsRequest;
use crate::docs::GeneratedDocsOutput;
use crate::gcp;
use crate::gcp::GcsUploadOptions;
use crate::gcp::CACHE_CONTROL_DO_NOT_CACHE;
use crate::iam::ReqIamExt;
use crate::ids::PackageName;
use crate::ids::PackagePath;
use crate::ids::ScopeName;
use crate::ids::Version;
use crate::metadata::PackageMetadata;
use crate::npm::generate_npm_version_manifest;
use crate::orama::OramaClient;
use crate::provenance;
use crate::publish::publish_task;
use crate::tarball::gcs_tarball_path;
use crate::util;
use crate::util::decode_json;
use crate::util::pagination;
use crate::util::search;
use crate::util::ApiResult;
use crate::util::CacheDuration;
use crate::util::RequestIdExt;
use crate::util::VersionOrLatest;
use crate::NpmUrl;
use crate::RegistryUrl;

use super::ApiCreatePackageRequest;
use super::ApiDependency;
use super::ApiDependent;
use super::ApiDownloadDataPoint;
use super::ApiError;
use super::ApiList;
use super::ApiMetrics;
use super::ApiPackage;
use super::ApiPackageDownloads;
use super::ApiPackageDownloadsRecentVersion;
use super::ApiPackageScore;
use super::ApiPackageVersion;
use super::ApiPackageVersionDocs;
use super::ApiPackageVersionSource;
use super::ApiPackageVersionWithUser;
use super::ApiProvenanceStatementRequest;
use super::ApiPublishingTask;
use super::ApiSource;
use super::ApiSourceDirEntry;
use super::ApiSourceDirEntryKind;
use super::ApiStats;
use super::ApiUpdatePackageGithubRepositoryRequest;
use super::ApiUpdatePackageRequest;
use super::ApiUpdatePackageVersionRequest;

const MAX_PUBLISH_TARBALL_SIZE: u64 = 20 * 1024 * 1024; // 20mb

pub struct PublishQueue(pub Option<gcp::Queue>);

pub fn package_router() -> Router<Body, ApiError> {
  Router::builder()
    .get("/", util::json(list_handler))
    .post("/", util::json(create_handler))
    .get("/:package", util::json(get_handler))
    .patch("/:package", util::auth(util::json(update_handler)))
    .delete("/:package", util::auth(delete_handler))
    .get(
      "/:package/versions",
      util::cache(CacheDuration::ONE_MINUTE, util::json(list_versions_handler)),
    )
    .get("/:package/dependents", util::json(list_dependents_handler))
    .get("/:package/downloads", util::json(get_downloads_handler))
    .get(
      "/:package/versions/:version",
      util::cache(CacheDuration::ONE_MINUTE, util::json(get_version_handler)),
    )
    .post(
      "/:package/versions/:version",
      util::auth(util::json(version_publish_handler)),
    )
    .patch(
      "/:package/versions/:version",
      util::auth(version_update_handler),
    )
    .post(
      "/:package/versions/:version/provenance",
      util::auth(version_provenance_statements_handler),
    )
    .get(
      "/:package/versions/:version/docs",
      util::cache(CacheDuration::ONE_MINUTE, util::json(get_docs_handler)),
    )
    .get(
      "/:package/versions/:version/docs/search",
      util::cache(
        CacheDuration::ONE_MINUTE,
        util::json(get_docs_search_handler),
      ),
    )
    .get(
      "/:package/versions/:version/docs/search_html",
      util::cache(
        CacheDuration::ONE_MINUTE,
        util::json(get_docs_search_html_handler),
      ),
    )
    .get(
      "/:package/versions/:version/source",
      util::cache(CacheDuration::ONE_MINUTE, util::json(get_source_handler)),
    )
    .get(
      "/:package/versions/:version/dependencies",
      util::json(list_dependencies_handler),
    )
    .get(
      "/:package/publishing_tasks",
      util::json(list_publishing_tasks_handler),
    )
    .get("/:package/score", util::json(get_score_handler))
    .build()
    .unwrap()
}

#[instrument(name = "GET /api/packages", skip(req), err, fields(query))]
pub async fn global_list_handler(
  req: Request<Body>,
) -> ApiResult<ApiList<ApiPackage>> {
  let db = req.data::<Database>().unwrap();

  let (start, limit) = pagination(&req);
  // Strip '@' prefix so scopes can still be searched.
  let maybe_search =
    search(&req).map(|query| query.strip_prefix('@').unwrap_or(query));
  if let Some(search) = maybe_search {
    Span::current().record("query", search);
  }

  let github_repo_id = req
    .query("gitHubRepoId")
    .map(|github_repo_id| {
      github_repo_id
        .parse::<i64>()
        .context("Failed to parse 'gitHubRepoId' query")
    })
    .transpose()?;

  let (total, packages) = db
    .list_packages(start, limit, maybe_search, github_repo_id)
    .await?;
  Ok(ApiList {
    items: packages.into_iter().map(ApiPackage::from).collect(),
    total,
  })
}

#[instrument(name = "GET /api/stats", skip(req), err)]
pub async fn global_stats_handler(req: Request<Body>) -> ApiResult<ApiStats> {
  let db = req.data::<Database>().unwrap();

  let (newest, updated, featured) = db.package_stats().await?;

  Ok(ApiStats {
    newest: newest.into_iter().map(ApiPackage::from).collect(),
    updated: updated.into_iter().map(ApiPackageVersion::from).collect(),
    featured: featured.into_iter().map(ApiPackage::from).collect(),
  })
}

#[instrument(name = "GET /api/metrics", skip(req), err)]
pub async fn global_metrics_handler(
  req: Request<Body>,
) -> ApiResult<ApiMetrics> {
  let db = req.data::<Database>().unwrap();
  let metrics = db.metrics().await?;
  Ok(metrics)
}

#[instrument(
  name = "GET /api/scopes/:scope/packages",
  skip(req),
  err,
  fields(scope)
)]
pub async fn list_handler(
  req: Request<Body>,
) -> ApiResult<ApiList<ApiPackage>> {
  let scope = req.param_scope()?;
  Span::current().record("scope", &field::display(&scope));

  let (start, limit) = pagination(&req);

  let db = req.data::<Database>().unwrap();
  db.get_scope(&scope).await?.ok_or(ApiError::ScopeNotFound)?;

  let iam = req.iam();
  let can_see_archived = iam.check_scope_admin_access(&scope).await.is_ok();
  let (total, packages) = db
    .list_packages_by_scope(&scope, can_see_archived, start, limit)
    .await?;

  Ok(ApiList {
    items: packages.into_iter().map(ApiPackage::from).collect(),
    total,
  })
}

#[instrument(
  name = "POST /api/scopes/:scope/packages",
  skip(req),
  err,
  fields(scope, package)
)]
pub async fn create_handler(mut req: Request<Body>) -> ApiResult<ApiPackage> {
  let scope = req.param_scope()?;
  Span::current().record("scope", &field::display(&scope));

  let ApiCreatePackageRequest {
    package: package_name,
  } = decode_json(&mut req).await?;
  Span::current().record("package", &field::display(&package_name));

  let iam = req.iam();
  iam.check_scope_write_access(&scope).await?;

  let db = req.data::<Database>().unwrap();

  if db.check_is_bad_word(&package_name.to_string()).await? {
    return Err(ApiError::PackageNameNotAllowed);
  }

  let res = db.create_package(&scope, &package_name).await?;
  let package = match res {
    CreatePackageResult::Ok(package) => package,
    CreatePackageResult::AlreadyExists => {
      return Err(ApiError::PackageAlreadyExists)
    }
    CreatePackageResult::PackageLimitExceeded(limit) => {
      return Err(ApiError::PackageLimitExceeded { limit })
    }
    CreatePackageResult::WeeklyPackageLimitExceeded(limit) => {
      return Err(ApiError::WeeklyPackageLimitExceeded { limit })
    }
  };

  let orama_client = req.data::<Option<OramaClient>>().unwrap();
  if let Some(orama_client) = orama_client {
    orama_client.upsert_package(&package, &Default::default());
  }

  Ok(ApiPackage::from((package, None, Default::default())))
}

#[instrument(
  name = "GET /api/scopes/:scope/packages/:package",
  skip(req),
  err,
  fields(scope, package)
)]
pub async fn get_handler(req: Request<Body>) -> ApiResult<ApiPackage> {
  let scope = req.param_scope()?;
  let package = req.param_package()?;

  Span::current().record("scope", &field::display(&scope));
  Span::current().record("package", &field::display(&package));

  let db = req.data::<Database>().unwrap();
  let res_package = db
    .get_package(&scope, &package)
    .await?
    .ok_or(ApiError::PackageNotFound)?;

  let mut api_package = ApiPackage::from(res_package);

  if let Some(latest_v) = &api_package.latest_version {
    // export next lines as a function ? Where ? utils ?
    let latest_version = Version::new(latest_v).unwrap();

    let deps = db
      .list_package_version_dependencies(&scope, &package, &latest_version)
      .await?;

    api_package.dependency_count = {
      let mut unique_deps = HashSet::new();
      deps
        .iter()
        .filter(|dep| unique_deps.insert(&dep.dependency_name))
        .count() as u64
    };
  }

  let dependent_count = db
    .count_package_dependents(
      crate::db::DependencyKind::Jsr,
      &format!("@{}/{}", scope, package),
    )
    .await?;
  api_package.dependent_count = dependent_count as u64;

  Ok(ApiPackage::from(api_package))
}

#[instrument(
  name = "PATCH /api/scopes/:scope/packages/:package",
  skip(req),
  err,
  fields(scope, package)
)]
pub async fn update_handler(mut req: Request<Body>) -> ApiResult<ApiPackage> {
  let scope = req.param_scope()?;
  let package_name = req.param_package()?;

  let body: ApiUpdatePackageRequest = decode_json(&mut req).await?;

  let db: &Database = req.data::<Database>().unwrap();
  let orama_client = req.data::<Option<OramaClient>>().unwrap();
  let github_oauth2_client = req.data::<GithubOauth2Client>().unwrap();

  let (package, repo, meta) = db
    .get_package(&scope, &package_name)
    .await?
    .ok_or(ApiError::PackageNotFound)?;

  let iam = req.iam();
  // Updating if a package is featured is allowed for admins, update package
  // description is allowed for all members, updating the repo
  // requires admin permissions because it extends who can publish new
  // versions (anyone with write access to the repo).
  if matches!(body, ApiUpdatePackageRequest::IsFeatured(_)) {
    iam.check_admin_access()?;
  } else if matches!(body, ApiUpdatePackageRequest::Description(_)) {
    iam.check_scope_write_access(&scope).await?;
  } else {
    iam.check_scope_admin_access(&scope).await?;
  }

  if package.is_archived
    && !matches!(body, ApiUpdatePackageRequest::IsArchived(_))
  {
    return Err(ApiError::PackageArchived);
  }

  match body {
    ApiUpdatePackageRequest::Description(description) => {
      let npm_url = &req.data::<NpmUrl>().unwrap().0;
      let buckets = req.data::<Buckets>().unwrap().clone();
      let package = update_description(
        db,
        npm_url,
        &buckets,
        orama_client,
        &scope,
        &package_name,
        description,
      )
      .await?;
      Ok(ApiPackage::from((package, repo, meta)))
    }
    ApiUpdatePackageRequest::GithubRepository(None) => {
      let package = db
        .delete_package_github_repository(&scope, &package_name)
        .await?;
      Ok(ApiPackage::from((package, None, meta)))
    }
    ApiUpdatePackageRequest::GithubRepository(Some(repo)) => {
      let current_user = iam.check_current_user_access()?;
      update_github_repository(
        current_user,
        db,
        github_oauth2_client,
        scope,
        package_name,
        repo,
      )
      .await
    }
    ApiUpdatePackageRequest::RuntimeCompat(runtime_compat) => {
      let runtime_compat: RuntimeCompat = runtime_compat.into();
      let package = db
        .update_package_runtime_compat(&scope, &package_name, &runtime_compat)
        .await?;
      if let Some(orama_client) = orama_client {
        orama_client.upsert_package(&package, &meta);
      }
      Ok(ApiPackage::from((package, repo, meta)))
    }
    ApiUpdatePackageRequest::IsFeatured(is_featured) => {
      let package = db
        .update_package_is_featured(&scope, &package_name, is_featured)
        .await?;
      Ok(ApiPackage::from((package, repo, meta)))
    }
    ApiUpdatePackageRequest::IsArchived(is_archived) => {
      let package = db
        .update_package_is_archived(&scope, &package_name, is_archived)
        .await?;

      if let Some(orama_client) = orama_client {
        if package.is_archived {
          orama_client.delete_package(&scope, &package.name);
        } else {
          orama_client.upsert_package(&package, &meta);
        }
      }

      Ok(ApiPackage::from((package, repo, meta)))
    }
  }
}

#[instrument(
  skip(db, npm_url, buckets, orama_client, scope, package_name),
  err,
  fields(description)
)]
async fn update_description(
  db: &Database,
  npm_url: &Url,
  buckets: &Buckets,
  orama_client: &Option<OramaClient>,
  scope: &ScopeName,
  package_name: &PackageName,
  description: String,
) -> Result<Package, ApiError> {
  let description = description.trim().replace('\n', " ").replace('\r', "");

  if description.len() > 250 {
    return Err(ApiError::MalformedRequest {
      msg: "description must not be longer than 250 characters".into(),
    });
  }

  if description.contains(|c: char| c.is_control()) {
    return Err(ApiError::MalformedRequest {
      msg: "description must not contain control characters".into(),
    });
  }

  let (package, _, meta) = db
    .update_package_description(scope, package_name, &description)
    .await?;

  if let Some(orama_client) = orama_client {
    orama_client.upsert_package(&package, &meta);
  }

  let npm_version_manifest_path =
    crate::gcs_paths::npm_version_manifest_path(scope, &package.name);
  let npm_version_manifest =
    generate_npm_version_manifest(db, npm_url, scope, &package.name).await?;
  let content = serde_json::to_vec_pretty(&npm_version_manifest)?;
  buckets
    .npm_bucket
    .upload(
      npm_version_manifest_path.into(),
      UploadTaskBody::Bytes(content.into()),
      GcsUploadOptions {
        content_type: Some("application/json".into()),
        cache_control: Some(CACHE_CONTROL_DO_NOT_CACHE.into()),
        gzip_encoded: false,
      },
    )
    .await?;

  Ok(package)
}

#[instrument(skip(db, scope, package, req), err, fields(repo.owner = req.owner, repo.name = req.name))]
async fn update_github_repository(
  user: &User,
  db: &Database,
  github_oauth2_client: &GithubOauth2Client,
  scope: ScopeName,
  package: PackageName,
  req: ApiUpdatePackageGithubRepositoryRequest,
) -> Result<ApiPackage, ApiError> {
  let gh_user_id = user.github_id.ok_or_else(|| {
    error!("user is not linked to a GitHub account");
    ApiError::InternalServerError
  })?;

  let ghid = db.get_github_identity(gh_user_id).await?;
  let mut new_ghid = ghid.into();
  let access_token =
    access_token(db, github_oauth2_client, &mut new_ghid).await?;
  let github_u2s_client = crate::github::GitHubUserClient::new(access_token);

  let repo = github_u2s_client
    .get_repo(&req.owner, &req.name)
    .await
    .map_err(|err| {
      if err.to_string().contains("SAML enforcement") {
        ApiError::GithubSamlEnforcement
      } else {
        err.into()
      }
    })?
    .ok_or(ApiError::GithubRepositoryNotFound)?;

  if repo.visibility != "public" {
    return Err(ApiError::GithubRepositoryNotPublic);
  }

  if !repo.permissions.push {
    return Err(ApiError::GithubRepositoryNotAuthorized);
  }

  let new_repo = NewGithubRepository {
    id: repo.id,
    owner: &repo.owner.login,
    name: &repo.name,
  };

  let (package, repo, score) = db
    .update_package_github_repository(&scope, &package, new_repo)
    .await?;

  Ok(ApiPackage::from((package, Some(repo), score)))
}

#[instrument(
  name = "GET /api/scopes/:scope/packages/:package/versions",
  skip(req),
  err,
  fields(scope, package)
)]
pub async fn list_versions_handler(
  req: Request<Body>,
) -> ApiResult<Vec<ApiPackageVersionWithUser>> {
  let scope = req.param_scope()?;
  let package = req.param_package()?;

  Span::current().record("scope", &field::display(&scope));
  Span::current().record("package", &field::display(&package));

  let db = req.data::<Database>().unwrap();

  db.get_package(&scope, &package)
    .await?
    .ok_or(ApiError::PackageNotFound)?;

  let versions = db
    .list_package_versions(&scope, &package)
    .await?
    .into_iter()
    .map(ApiPackageVersionWithUser::from)
    .collect::<Vec<_>>();

  Ok(versions)
}

#[instrument(
  name = "DELETE /api/scopes/:scope/packages/:package",
  skip(req),
  err,
  fields(scope, package)
)]
pub async fn delete_handler(req: Request<Body>) -> ApiResult<Response<Body>> {
  let scope = req.param_scope()?;
  let package = req.param_package()?;

  let db: &Database = req.data::<Database>().unwrap();

  let _ = db
    .get_package(&scope, &package)
    .await?
    .ok_or(ApiError::PackageNotFound)?;

  let iam = req.iam();
  iam.check_scope_admin_access(&scope).await?;

  let deleted = db.delete_package(&scope, &package).await?;
  if !deleted {
    return Err(ApiError::PackageNotEmpty);
  }

  let orama_client = req.data::<Option<OramaClient>>().unwrap();
  if let Some(orama_client) = orama_client {
    orama_client.delete_package(&scope, &package);
  }

  let res = Response::builder()
    .status(StatusCode::NO_CONTENT)
    .body(Body::empty())
    .unwrap();
  Ok(res)
}

#[instrument(
  name = "GET /api/scopes/:scope/packages/:package/versions/:version",
  skip(req),
  err,
  fields(scope, package, version)
)]
pub async fn get_version_handler(
  req: Request<Body>,
) -> ApiResult<ApiPackageVersion> {
  let scope = req.param_scope()?;
  let package = req.param_package()?;
  let version = req.param_version_or_latest()?;
  Span::current().record("scope", &field::display(&scope));
  Span::current().record("package", &field::display(&package));
  Span::current().record("version", &field::display(&version));

  let db = req.data::<Database>().unwrap();
  let _ = db
    .get_package(&scope, &package)
    .await?
    .ok_or(ApiError::PackageNotFound)?;

  let maybe_version = match version {
    VersionOrLatest::Version(version) => {
      db.get_package_version(&scope, &package, &version).await?
    }
    VersionOrLatest::Latest => {
      db.get_latest_unyanked_version_for_package(&scope, &package)
        .await?
    }
  };

  let version = maybe_version.ok_or(ApiError::PackageVersionNotFound)?;

  Ok(ApiPackageVersion::from(version))
}

#[instrument(
  name = "POST /api/scopes/:scope/packages/:package/versions/:version",
  skip(req),
  err,
  fields(scope, package, version)
)]
pub async fn version_publish_handler(
  req: Request<Body>,
) -> ApiResult<ApiPublishingTask> {
  let package_scope = req.param_scope()?;
  let package_name = req.param_package()?;
  let package_version = req.param_version()?;
  Span::current().record("scope", &field::display(&package_scope));
  Span::current().record("package", &field::display(&package_name));
  Span::current().record("version", &field::display(&package_version));
  let config_file =
    PackagePath::try_from(&**req.query("config").ok_or_else(|| {
      let msg = "Missing query parameter 'config'".into();
      ApiError::MalformedRequest { msg }
    })?)
    .map_err(|err| {
      let msg = format!(
        "failed to parse query parameter 'config' with value '{}': {err}",
        req.query("config").unwrap()
      )
      .into();
      ApiError::MalformedRequest { msg }
    })?;

  // If there is a content-length header, check it isn't too big.
  // We don't rely on this, we will also check MAX_PAYLOAD_SIZE later.
  if let Some(size) = req.body().size_hint().upper() {
    if size > MAX_PUBLISH_TARBALL_SIZE {
      return Err(ApiError::TarballSizeLimitExceeded {
        size,
        max_size: MAX_PUBLISH_TARBALL_SIZE,
      });
    }
  }

  // Ensure the upload is gzip encoded.
  match req.headers().get(hyper::header::CONTENT_ENCODING) {
    Some(val) if val == "gzip" => (),
    _ => return Err(ApiError::MissingGzipContentEncoding),
  }

  let db = req.data::<Database>().unwrap().clone();
  let buckets = req.data::<Buckets>().unwrap().clone();
  let registry_url = req.data::<RegistryUrl>().unwrap().0.clone();
  let npm_url = req.data::<NpmUrl>().unwrap().0.clone();
  let publish_queue = req.data::<PublishQueue>().unwrap().0.clone();
  let orama_client = req.data::<Option<OramaClient>>().unwrap().clone();

  let iam = req.iam();
  let (access_restriction, user_id) = iam
    .check_publish_access(&package_scope, &package_name, &package_version)
    .await?;

  let (package, _, _) = db
    .get_package(&package_scope, &package_name)
    .await?
    .ok_or(ApiError::PackageNotFound)?;

  if package.is_archived {
    return Err(ApiError::PackageArchived);
  }

  let res = db
    .create_publishing_task(NewPublishingTask {
      user_id,
      package_scope: &package.scope,
      package_name: &package.name,
      package_version: &package_version,
      config_file: &config_file,
    })
    .await?;
  let publishing_task = match res {
    CreatePublishingTaskResult::Created(publishing_task) => publishing_task,
    CreatePublishingTaskResult::Exists(task) => {
      return Err(ApiError::DuplicateVersionPublish {
        task: Box::new(task.into()),
      })
    }
    CreatePublishingTaskResult::WeeklyPublishAttemptsLimitExceeded(limit) => {
      return Err(ApiError::WeeklyPublishAttemptsLimitExceeded { limit })
    }
  };

  let gcs_path = gcs_tarball_path(publishing_task.id);

  let body = req.into_body();
  let total_size = Arc::new(AtomicU64::new(0));
  let total_size_ = total_size.clone();

  let hash = Arc::new(Mutex::new(Some(sha2::Sha256::new())));

  let hash_ = hash.clone();
  let stream = body.map(move |res| match res {
    Ok(bytes) => {
      hash_.lock().unwrap().as_mut().unwrap().update(&bytes);
      total_size_.fetch_add(bytes.len() as u64, Ordering::SeqCst);
      if total_size_.load(Ordering::SeqCst) > MAX_PUBLISH_TARBALL_SIZE {
        Err(io::Error::new(io::ErrorKind::Other, "Payload too large"))
      } else {
        Ok(bytes)
      }
    }
    Err(err) => Err(io::Error::new(io::ErrorKind::Other, err)),
  });

  let upload_result = buckets
    .publishing_bucket
    .upload(
      gcs_path.into(),
      UploadTaskBody::Stream(Box::new(stream)),
      GcsUploadOptions {
        content_type: Some("application/x-tar".into()),
        cache_control: None,
        gzip_encoded: true,
      },
    )
    .await;

  let hash = hash.lock().unwrap().take().unwrap().finalize();
  let hash = format!("sha256-{:02x}", hash);
  if let Some(tarball_hash) = access_restriction.tarball_hash {
    if tarball_hash != hash {
      error!(
        "Tarball hash mismatch: expected {}, got {}",
        tarball_hash, hash
      );
      return Err(ApiError::MissingPermission);
    }
  }

  // If the upload failed due to the size limit, we can cancel the task.
  let total_size = total_size.load(Ordering::SeqCst);
  if total_size > MAX_PUBLISH_TARBALL_SIZE {
    return Err(ApiError::TarballSizeLimitExceeded {
      size: total_size,
      max_size: MAX_PUBLISH_TARBALL_SIZE,
    });
  }

  // Otherwise, we can just propagate the error.
  upload_result?;

  if let Some(queue) = publish_queue {
    let body = serde_json::to_vec(&publishing_task.id).unwrap();
    queue.task_buffer(None, Some(body.into())).await?;
  } else {
    let span = Span::current();
    let fut = publish_task(
      publishing_task.id,
      buckets.clone(),
      registry_url,
      npm_url,
      db,
      orama_client,
    )
    .instrument(span);
    tokio::spawn(fut);
  }

  Ok(publishing_task.into())
}

#[instrument(
  name = "POST /api/scopes/:scope/packages/:package/versions/:version/provenance",
  skip(req),
  err,
  fields(scope, package, version)
)]
pub async fn version_provenance_statements_handler(
  mut req: Request<Body>,
) -> ApiResult<Response<Body>> {
  let scope = req.param_scope()?;
  let package = req.param_package()?;
  let version = req.param_version()?;

  Span::current().record("scope", &field::display(&scope));
  Span::current().record("package", &field::display(&package));
  Span::current().record("version", &field::display(&version));

  let body: ApiProvenanceStatementRequest = decode_json(&mut req).await?;

  let db = req.data::<Database>().unwrap();
  let orama_client = req.data::<Option<OramaClient>>().unwrap().clone();

  let iam = req.iam();
  iam.check_publish_access(&scope, &package, &version).await?;

  let name = format!("pkg:jsr/@{}/{}@{}", scope, package, version);
  let rekor_log_id = provenance::verify(name, body.bundle)?;

  db.insert_provenance_statement(&scope, &package, &version, &rekor_log_id)
    .await?;

  if let Some(orama_client) = orama_client {
    let (package, _, meta) = db
      .get_package(&scope, &package)
      .await?
      .ok_or_else(|| ApiError::InternalServerError)?;
    orama_client.upsert_package(&package, &meta);
  }

  Ok(
    Response::builder()
      .status(StatusCode::NO_CONTENT)
      .body(Body::empty())
      .unwrap(),
  )
}

#[instrument(
  name = "PATCH /api/scopes/:scope/packages/:package/versions/:version",
  skip(req),
  err,
  fields(scope, package, version)
)]
pub async fn version_update_handler(
  mut req: Request<Body>,
) -> ApiResult<Response<Body>> {
  let scope = req.param_scope()?;
  let package = req.param_package()?;
  let version = req.param_version()?;
  Span::current().record("scope", &field::display(&scope));
  Span::current().record("package", &field::display(&package));
  Span::current().record("version", &field::display(&version));

  // WARNING: if an additional option gets added, then yanked time rendering needs to be changed in package/versions.tsx
  let body: ApiUpdatePackageVersionRequest = decode_json(&mut req).await?;

  let db = req.data::<Database>().unwrap();
  let buckets = req.data::<Buckets>().unwrap().clone();
  let npm_url = &req.data::<NpmUrl>().unwrap().0;

  let iam = req.iam();
  iam.check_scope_admin_access(&scope).await?;

  db.yank_package_version(&scope, &package, &version, body.yanked)
    .await?;

  let package_metadata_path =
    crate::gcs_paths::package_metadata(&scope, &package);
  let package_metadata = PackageMetadata::create(db, &scope, &package).await?;

  let content = serde_json::to_vec_pretty(&package_metadata)?;
  buckets
    .modules_bucket
    .upload(
      package_metadata_path.into(),
      UploadTaskBody::Bytes(content.into()),
      GcsUploadOptions {
        content_type: Some("application/json".into()),
        cache_control: Some(CACHE_CONTROL_DO_NOT_CACHE.into()),
        gzip_encoded: false,
      },
    )
    .await
    .unwrap();

  let npm_version_manifest_path =
    crate::gcs_paths::npm_version_manifest_path(&scope, &package);
  let npm_version_manifest =
    generate_npm_version_manifest(db, npm_url, &scope, &package).await?;
  let content = serde_json::to_vec_pretty(&npm_version_manifest)?;
  buckets
    .npm_bucket
    .upload(
      npm_version_manifest_path.into(),
      UploadTaskBody::Bytes(content.into()),
      GcsUploadOptions {
        content_type: Some("application/json".into()),
        cache_control: Some(CACHE_CONTROL_DO_NOT_CACHE.into()),
        gzip_encoded: false,
      },
    )
    .await?;

  Ok(
    Response::builder()
      .status(StatusCode::NO_CONTENT)
      .body(Body::empty())
      .unwrap(),
  )
}

#[instrument(
  name = "GET /api/scopes/:scope/packages/:package/versions/:version/docs",
  skip(req),
  err,
  fields(scope, package, version, all_symbols, entrypoint, symbol)
)]
pub async fn get_docs_handler(
  req: Request<Body>,
) -> ApiResult<ApiPackageVersionDocs> {
  let scope = req.param_scope()?;
  let package_name = req.param_package()?;
  let version_or_latest = req.param_version_or_latest()?;
  Span::current().record("scope", &field::display(&scope));
  Span::current().record("package", &field::display(&package_name));
  Span::current().record("version", &field::display(&version_or_latest));
  let all_symbols = req.query("all_symbols").is_some();
  Span::current().record("all_symbols", &field::display(&all_symbols));
  let entrypoint = req.query("entrypoint").and_then(|s| match s.as_str() {
    "" => None,
    s => Some(s),
  });
  Span::current()
    .record("entrypoint", &field::display(&entrypoint.unwrap_or("")));

  let symbol = req
    .query("symbol")
    .and_then(|s| match s.as_str() {
      "" => None,
      s => Some(urlencoding::decode(s)),
    })
    .transpose()?;
  Span::current()
    .record("symbol", &field::display(&symbol.as_deref().unwrap_or("")));

  if all_symbols && (entrypoint.is_some() || symbol.is_some()) {
    return Err(ApiError::MalformedRequest {
      msg: "Cannot specify both all_symbols and entrypoint".into(),
    });
  }

  let db = req.data::<Database>().unwrap();
  let buckets = req.data::<Buckets>().unwrap();
  let (package, repo, _) = db
    .get_package(&scope, &package_name)
    .await?
    .ok_or(ApiError::PackageNotFound)?;

  let maybe_version = match &version_or_latest {
    VersionOrLatest::Version(version) => {
      db.get_package_version(&scope, &package_name, version)
        .await?
    }
    VersionOrLatest::Latest => {
      db.get_latest_unyanked_version_for_package(&scope, &package_name)
        .await?
    }
  };
  let version = maybe_version.ok_or(ApiError::PackageVersionNotFound)?;

  let docs_path =
    crate::gcs_paths::docs_v1_path(&scope, &package_name, &version.version);
  let doc_nodes_fut = buckets.docs_bucket.download(docs_path.into());
  let readme_fut = if !all_symbols && entrypoint.is_none() && symbol.is_none() {
    if let Some(readme_path) = &version.readme_path {
      let gcs_path = crate::gcs_paths::file_path(
        &scope,
        &package_name,
        &version.version,
        readme_path,
      )
      .into();
      Either::Left(buckets.modules_bucket.download(gcs_path))
    } else {
      Either::Right(futures::future::ready(Ok(None)))
    }
  } else {
    Either::Right(futures::future::ready(Ok(None)))
  };

  let (docs, readme) =
    futures::future::try_join(doc_nodes_fut, readme_fut).await?;
  let docs = docs.ok_or_else(|| {
    error!(
      "docs not found for {}/{}/{}",
      scope, package_name, version.version
    );
    ApiError::InternalServerError
  })?;
  let doc_nodes: DocNodesByUrl =
    serde_json::from_slice(&docs).context("failed to parse doc nodes")?;
  let readme = readme.and_then(|readme| {
    std::str::from_utf8(&readme).ok().map(ToOwned::to_owned)
  });

  let docs_info = crate::docs::get_docs_info(&version.exports, entrypoint);

  if entrypoint.is_some() && docs_info.entrypoint_url.is_none() {
    return Err(ApiError::EntrypointOrSymbolNotFound);
  }

  let registry_url = req.data::<RegistryUrl>().unwrap().0.to_string();

  let req = match (docs_info.entrypoint_url, symbol) {
    _ if all_symbols => DocsRequest::AllSymbols,
    (Some(entrypoint), None) => DocsRequest::File(entrypoint),
    (Some(entrypoint), Some(symbol)) => {
      DocsRequest::Symbol(entrypoint, symbol.into())
    }
    (None, Some(symbol)) => {
      if let Some(entrypoint_url) = docs_info.main_entrypoint.clone() {
        DocsRequest::Symbol(entrypoint_url, symbol.into())
      } else {
        return Err(ApiError::EntrypointOrSymbolNotFound);
      }
    }
    (None, None) => DocsRequest::Index,
  };

  let docs = crate::docs::generate_docs_html(
    doc_nodes,
    docs_info.main_entrypoint,
    docs_info.rewrite_map,
    req,
    scope.clone(),
    package_name.clone(),
    version.version.clone(),
    version_or_latest == VersionOrLatest::Latest,
    repo,
    readme,
    package.runtime_compat,
    registry_url,
  )
  .map_err(|e| {
    error!("failed to generate docs: {}", e);
    ApiError::InternalServerError
  })?
  .ok_or(ApiError::EntrypointOrSymbolNotFound)?;

  match docs {
    GeneratedDocsOutput::Docs(docs) => Ok(ApiPackageVersionDocs::Content {
      css: Cow::Borrowed(deno_doc::html::STYLESHEET),
      comrak_css: Cow::Borrowed(deno_doc::html::comrak::COMRAK_STYLESHEET),
      script: Cow::Borrowed(deno_doc::html::SCRIPT_JS),
      breadcrumbs: docs.breadcrumbs,
      toc: docs.toc,
      main: docs.main,
      version: ApiPackageVersion::from(version),
    }),
    GeneratedDocsOutput::Redirect(href) => {
      Ok(ApiPackageVersionDocs::Redirect { symbol: href })
    }
  }
}

#[instrument(
  name = "GET /api/scopes/:scope/packages/:package/versions/:version/docs/search",
  skip(req),
  err,
  fields(scope, package, version, all_symbols, entrypoint, symbol)
)]
pub async fn get_docs_search_handler(
  req: Request<Body>,
) -> ApiResult<serde_json::Value> {
  let scope = req.param_scope()?;
  let package_name = req.param_package()?;
  let version_or_latest = req.param_version_or_latest()?;
  Span::current().record("scope", &field::display(&scope));
  Span::current().record("package", &field::display(&package_name));
  Span::current().record("version", &field::display(&version_or_latest));

  let db = req.data::<Database>().unwrap();
  let buckets = req.data::<Buckets>().unwrap();
  let (package, repo, _) = db
    .get_package(&scope, &package_name)
    .await?
    .ok_or(ApiError::PackageNotFound)?;

  let maybe_version = match &version_or_latest {
    VersionOrLatest::Version(version) => {
      db.get_package_version(&scope, &package_name, version)
        .await?
    }
    VersionOrLatest::Latest => {
      db.get_latest_unyanked_version_for_package(&scope, &package_name)
        .await?
    }
  };
  let version = maybe_version.ok_or(ApiError::PackageVersionNotFound)?;

  let docs_path =
    crate::gcs_paths::docs_v1_path(&scope, &package_name, &version.version);
  let docs = buckets.docs_bucket.download(docs_path.into()).await?;
  let docs = docs.ok_or_else(|| {
    error!(
      "docs not found for {}/{}/{}",
      scope, package_name, version.version
    );
    ApiError::InternalServerError
  })?;
  let doc_nodes: DocNodesByUrl =
    serde_json::from_slice(&docs).context("failed to parse doc nodes")?;

  let docs_info = crate::docs::get_docs_info(&version.exports, None);

  let registry_url = req.data::<RegistryUrl>().unwrap().0.to_string();

  let ctx = crate::docs::get_generate_ctx(
    doc_nodes,
    docs_info.main_entrypoint,
    docs_info.rewrite_map,
    scope.clone(),
    package_name.clone(),
    version.version.clone(),
    version_or_latest == VersionOrLatest::Latest,
    repo,
    false,
    package.runtime_compat,
    registry_url,
  );

  let search_index = deno_doc::html::generate_search_index(&ctx);

  Ok(search_index)
}

#[instrument(
  name = "GET /api/scopes/:scope/packages/:package/versions/:version/docs/search_html",
  skip(req),
  err,
  fields(scope, package, version, all_symbols, entrypoint, symbol)
)]
pub async fn get_docs_search_html_handler(
  req: Request<Body>,
) -> ApiResult<String> {
  let scope = req.param_scope()?;
  let package_name = req.param_package()?;
  let version_or_latest = req.param_version_or_latest()?;
  Span::current().record("scope", &field::display(&scope));
  Span::current().record("package", &field::display(&package_name));
  Span::current().record("version", &field::display(&version_or_latest));

  let db = req.data::<Database>().unwrap();
  let buckets = req.data::<Buckets>().unwrap();
  let (package, repo, _) = db
    .get_package(&scope, &package_name)
    .await?
    .ok_or(ApiError::PackageNotFound)?;

  let maybe_version = match &version_or_latest {
    VersionOrLatest::Version(version) => {
      db.get_package_version(&scope, &package_name, version)
        .await?
    }
    VersionOrLatest::Latest => {
      db.get_latest_unyanked_version_for_package(&scope, &package_name)
        .await?
    }
  };
  let version = maybe_version.ok_or(ApiError::PackageVersionNotFound)?;

  let docs_path =
    crate::gcs_paths::docs_v1_path(&scope, &package_name, &version.version);
  let docs = buckets.docs_bucket.download(docs_path.into()).await?;
  let docs = docs.ok_or_else(|| {
    error!(
      "docs not found for {}/{}/{}",
      scope, package_name, version.version
    );
    ApiError::InternalServerError
  })?;
  let doc_nodes: DocNodesByUrl =
    serde_json::from_slice(&docs).context("failed to parse doc nodes")?;

  let docs_info = crate::docs::get_docs_info(&version.exports, None);

  let registry_url = req.data::<RegistryUrl>().unwrap().0.to_string();

  let docs = crate::docs::generate_docs_html(
    doc_nodes,
    docs_info.main_entrypoint,
    docs_info.rewrite_map,
    DocsRequest::AllSymbols,
    scope.clone(),
    package_name.clone(),
    version.version.clone(),
    version_or_latest == VersionOrLatest::Latest,
    repo,
    None,
    package.runtime_compat,
    registry_url,
  )
  .map_err(|e| {
    error!("failed to generate docs: {}", e);
    ApiError::InternalServerError
  })?
  .unwrap();

  let search = match docs {
    GeneratedDocsOutput::Docs(docs) => docs.main,
    GeneratedDocsOutput::Redirect(_) => unreachable!(),
  };

  Ok(search)
}

#[instrument(
  name = "GET /api/scopes/:scope/packages/:package/versions/:version/source",
  skip(req),
  err,
  fields(scope, package, version, path)
)]
pub async fn get_source_handler(
  req: Request<Body>,
) -> ApiResult<ApiPackageVersionSource> {
  let scope = req.param_scope()?;
  let package = req.param_package()?;
  let version_or_latest = req.param_version_or_latest()?;
  let path = req.query("path").cloned().unwrap_or("/".to_string());

  Span::current().record("scope", &field::display(&scope));
  Span::current().record("package", &field::display(&package));
  Span::current().record("version", &field::display(&version_or_latest));
  Span::current().record("path", &field::display(&path));

  let db = req.data::<Database>().unwrap();
  let buckets = req.data::<Buckets>().unwrap();
  let _ = db
    .get_package(&scope, &package)
    .await?
    .ok_or(ApiError::PackageNotFound)?;

  let maybe_version = match &version_or_latest {
    VersionOrLatest::Version(version) => {
      db.get_package_version(&scope, &package, version).await?
    }
    VersionOrLatest::Latest => {
      db.get_latest_unyanked_version_for_package(&scope, &package)
        .await?
    }
  };
  let version = maybe_version.ok_or(ApiError::PackageVersionNotFound)?;

  let file = if path == "meta.json" {
    let source_file_path = crate::gcs_paths::package_metadata(&scope, &package);
    buckets
      .modules_bucket
      .download(source_file_path.into())
      .await?
  } else if path == format!("{}_meta.json", version.version) {
    let source_file_path =
      crate::gcs_paths::version_metadata(&scope, &package, &version.version);
    buckets
      .modules_bucket
      .download(source_file_path.into())
      .await?
  } else if path != "/" {
    let package_path = PackagePath::try_from(path.as_str()).map_err(|err| {
      let msg = format!("failed to parse path parameter 'path': {err}").into();
      ApiError::MalformedRequest { msg }
    })?;

    let source_file_path = crate::gcs_paths::file_path(
      &scope,
      &package,
      &version.version,
      &package_path,
    );
    buckets
      .modules_bucket
      .download(source_file_path.into())
      .await?
  } else {
    None
  };

  let path_buf = std::path::PathBuf::from(path);

  let source = if let Some(file) = file {
    let size = file.len();

    let highlighter = deno_doc::html::comrak::ComrakHighlightWrapperAdapter(
      Some(Arc::new(crate::tree_sitter::ComrakAdapter {
        show_line_numbers: true,
      })),
    );

    let view = if let Ok(file) = String::from_utf8(file.to_vec()) {
      let mut out = vec![];
      highlighter.write_pre_tag(&mut out, Default::default())?;
      highlighter.write_code_tag(&mut out, Default::default())?;
      highlighter.write_highlighted(
        &mut out,
        path_buf
          .extension()
          .map(|ext| ext.to_string_lossy())
          .as_deref(),
        &file,
      )?;
      out.extend(b"</code></pre>");

      Some(String::from_utf8(out).context("File is not valid utf8")?)
    } else {
      None
    };

    ApiSource::File { size, view }
  } else {
    let files = db
      .list_package_files(&scope, &package, &version.version)
      .await?;

    let mut entries = indexmap::IndexMap::new();

    for file in files {
      let file_path = std::path::PathBuf::from(&*file.path);
      let Ok(stripped_file_path) = file_path.strip_prefix(&path_buf) else {
        continue;
      };

      let mut path_parts = stripped_file_path.iter();

      let top_entry_name = path_parts
        .next()
        .map(|m_path| m_path.to_string_lossy().to_string());

      let Some(top_entry_name) = top_entry_name else {
        continue;
      };

      let entry = entries.entry(top_entry_name.clone());
      let dir_entry = entry.or_insert_with(|| ApiSourceDirEntry {
        name: top_entry_name,
        size: 0,
        kind: if path_parts.next().is_some() {
          ApiSourceDirEntryKind::Dir
        } else {
          ApiSourceDirEntryKind::File
        },
      });

      dir_entry.size += file.size as usize;
    }

    if entries.is_empty() {
      return Err(ApiError::PackagePathNotFound);
    }

    entries.sort_by(|_a_key, a, _b_key, b| {
      a.kind.cmp(&b.kind).then_with(|| a.name.cmp(&b.name))
    });

    ApiSource::Dir {
      entries: entries.into_values().collect(),
    }
  };

  Ok(ApiPackageVersionSource {
    version: ApiPackageVersion::from(version),
    css: Cow::Borrowed(deno_doc::html::STYLESHEET),
    comrak_css: Cow::Borrowed(deno_doc::html::comrak::COMRAK_STYLESHEET),
    script: Cow::Borrowed(deno_doc::html::SCRIPT_JS),
    source,
  })
}

#[instrument(
  name = "GET /api/scopes/:scope/packages/:package/dependents",
  skip(req),
  err,
  fields(scope, package)
)]
pub async fn list_dependents_handler(
  req: Request<Body>,
) -> ApiResult<ApiList<ApiDependent>> {
  let scope = req.param_scope()?;
  let package = req.param_package()?;
  Span::current().record("scope", &field::display(&scope));
  Span::current().record("package", &field::display(&package));

  let (start, limit) = pagination(&req);
  let versions_per_package_limit = req
    .query("versions_per_package_limit")
    .and_then(|page| page.parse::<i64>().ok())
    .unwrap_or(10)
    .clamp(1, 10);

  let db = req.data::<Database>().unwrap();
  db.get_package(&scope, &package)
    .await?
    .ok_or(ApiError::PackageNotFound)?;

  let (total, deps) = db
    .list_package_dependents(
      crate::db::DependencyKind::Jsr,
      &format!("@{}/{}", scope, package),
      start,
      limit,
      versions_per_package_limit,
    )
    .await?;
  let dependents = deps.into_iter().map(ApiDependent::from).collect::<Vec<_>>();

  Ok(ApiList {
    items: dependents,
    total,
  })
}

#[instrument(
  name = "GET /api/scopes/:scope/packages/:package/downloads",
  skip(req),
  err,
  fields(scope, package)
)]
pub async fn get_downloads_handler(
  req: Request<Body>,
) -> ApiResult<ApiPackageDownloads> {
  let scope = req.param_scope()?;
  let package = req.param_package()?;
  Span::current().record("scope", &field::display(&scope));
  Span::current().record("package", &field::display(&package));

  let db = req.data::<Database>().unwrap();
  db.get_package(&scope, &package)
    .await?
    .ok_or(ApiError::PackageNotFound)?;

  let current = Utc::now();
  let start = current - chrono::Duration::days(90);

  let total = db
    .get_package_downloads_24h(&scope, &package, start, current)
    .await?;

  let recent_versions = db
    .list_latest_unyanked_versions_for_package(&scope, &package, 5)
    .await?;

  let versions = futures::stream::iter(recent_versions.into_iter())
    .then(|version| async {
      let res = db
        .get_package_version_downloads_24h(
          &scope, &package, &version, start, current,
        )
        .await;
      (version, res)
    })
    .collect::<Vec<_>>()
    .await;

  let mut recent_versions = Vec::with_capacity(versions.len());
  for (version, downloads) in versions {
    let downloads = downloads?
      .into_iter()
      .map(ApiDownloadDataPoint::from)
      .collect();
    recent_versions
      .push(ApiPackageDownloadsRecentVersion { version, downloads });
  }

  Ok(ApiPackageDownloads {
    total: total.into_iter().map(ApiDownloadDataPoint::from).collect(),
    recent_versions,
  })
}

#[instrument(
  name = "GET /api/scopes/:scope/packages/:package/versions/:version/dependencies",
  skip(req),
  err,
  fields(scope, package, version)
)]
pub async fn list_dependencies_handler(
  req: Request<Body>,
) -> ApiResult<Vec<ApiDependency>> {
  let scope = req.param_scope()?;
  let package = req.param_package()?;
  let version = req.param_version()?;
  Span::current().record("scope", &field::display(&scope));
  Span::current().record("package", &field::display(&package));
  Span::current().record("version", &field::display(&version));

  let db = req.data::<Database>().unwrap();

  db.get_package_version(&scope, &package, &version)
    .await?
    .ok_or(ApiError::PackageVersionNotFound)?;

  let deps = db
    .list_package_version_dependencies(&scope, &package, &version)
    .await?;
  let deps = deps
    .into_iter()
    .map(ApiDependency::from)
    .collect::<Vec<_>>();

  Ok(deps)
}

#[instrument(
  name = "GET /api/scopes/:scope/packages/:package/publishing_tasks",
  skip(req),
  err,
  fields(scope, package)
)]
pub async fn list_publishing_tasks_handler(
  req: Request<Body>,
) -> ApiResult<Vec<ApiPublishingTask>> {
  let scope = req.param_scope()?;
  let package = req.param_package()?;
  Span::current().record("scope", &field::display(&scope));
  Span::current().record("package", &field::display(&package));

  let db = req.data::<Database>().unwrap();

  let iam = req.iam();
  iam.check_scope_write_access(&scope).await?;

  let publishing_tasks = db
    .list_publishing_tasks_for_package(&scope, &package)
    .await?;

  Ok(
    publishing_tasks
      .into_iter()
      .map(|task| task.into())
      .collect(),
  )
}

#[instrument(
  name = "GET /api/scopes/:scope/packages/:package/score",
  skip(req),
  err,
  fields(scope, package)
)]
pub async fn get_score_handler(
  req: Request<Body>,
) -> ApiResult<ApiPackageScore> {
  let scope = req.param_scope()?;
  let package = req.param_package()?;
  Span::current().record("scope", &field::display(&scope));
  Span::current().record("package", &field::display(&package));

  let db = req.data::<Database>().unwrap();
  let (pkg, _, meta) = db
    .get_package(&scope, &package)
    .await?
    .ok_or(ApiError::PackageNotFound)?;

  Ok(ApiPackageScore::from((&meta, &pkg)))
}

#[cfg(test)]
mod test {
  use hyper::Body;
  use hyper::StatusCode;
  use serde_json::json;

  use crate::api::ApiDependency;
  use crate::api::ApiDependencyKind;
  use crate::api::ApiDependent;
  use crate::api::ApiList;
  use crate::api::ApiMetrics;
  use crate::api::ApiPackage;
  use crate::api::ApiPackageScore;
  use crate::api::ApiPackageVersion;
  use crate::api::ApiPackageVersionDocs;
  use crate::api::ApiPackageVersionSource;
  use crate::api::ApiSource;
  use crate::api::ApiSourceDirEntry;
  use crate::api::ApiSourceDirEntryKind;
  use crate::db::CreatePackageResult;
  use crate::db::CreatePublishingTaskResult;
  use crate::db::ExportsMap;
  use crate::db::NewGithubRepository;
  use crate::db::NewPackageVersion;
  use crate::db::NewPublishingTask;
  use crate::db::NewScopeInvite;
  use crate::db::PackagePublishPermission;
  use crate::db::Permission;
  use crate::db::Permissions;
  use crate::db::PublishingTaskStatus;
  use crate::db::TokenType;
  use crate::ids::PackageName;
  use crate::ids::PackagePath;
  use crate::ids::ScopeName;
  use crate::ids::Version;
  use crate::publish::tests::create_mock_tarball;
  use crate::publish::tests::process_tarball_setup;
  use crate::publish::tests::process_tarball_setup2;
  use crate::token::create_token;
  use crate::util::test::ApiResultExt;
  use crate::util::test::TestSetup;

  #[tokio::test]
  async fn test_packages_list() {
    let mut t = TestSetup::new().await;

    let scope = t.scope.scope.clone();

    for i in 1..=150 {
      let name = PackageName::new(format!("foo{i}")).unwrap();
      let res = t
        .ephemeral_database
        .create_package(&scope, &name)
        .await
        .unwrap();

      t.ephemeral_database
        .update_package_github_repository(
          &scope,
          &name,
          NewGithubRepository {
            id: i % 10,
            owner: "foo",
            name: "bar",
          },
        )
        .await
        .unwrap();
      assert!(matches!(res, CreatePackageResult::Ok(_)));
    }

    let mut resp = t.http().get("/api/packages").call().await.unwrap();
    let packages: ApiList<ApiPackage> = resp.expect_ok().await;
    assert_eq!(packages.items.len(), 100);

    let mut resp = t.http().get("/api/packages?page=0").call().await.unwrap();
    let packages: ApiList<ApiPackage> = resp.expect_ok().await;
    assert_eq!(packages.items.len(), 100);

    let mut resp = t.http().get("/api/packages?page=2").call().await.unwrap();
    let packages: ApiList<ApiPackage> = resp.expect_ok().await;
    assert_eq!(packages.items.len(), 50);

    let mut resp = t.http().get("/api/packages?page=3").call().await.unwrap();
    let packages: ApiList<ApiPackage> = resp.expect_ok().await;
    assert_eq!(packages.items.len(), 0);

    let mut resp = t.http().get("/api/packages?limit=1").call().await.unwrap();
    let packages: ApiList<ApiPackage> = resp.expect_ok().await;
    assert_eq!(packages.items.len(), 1);

    let mut resp = t.http().get("/api/packages?limit=53").call().await.unwrap();
    let packages: ApiList<ApiPackage> = resp.expect_ok().await;
    assert_eq!(packages.items.len(), 53);

    let mut resp = t
      .http()
      .get("/api/packages?limit=400")
      .call()
      .await
      .unwrap();
    let packages: ApiList<ApiPackage> = resp.expect_ok().await;
    assert_eq!(packages.items.len(), 100);

    let mut resp = t
      .http()
      .get("/api/packages?page=2&limit=23")
      .call()
      .await
      .unwrap();
    let packages: ApiList<ApiPackage> = resp.expect_ok().await;
    assert_eq!(packages.items.len(), 23);

    let mut resp = t
      .http()
      .get("/api/packages?query=bar")
      .call()
      .await
      .unwrap();
    let packages: ApiList<ApiPackage> = resp.expect_ok().await;
    assert_eq!(packages.items.len(), 0);

    let mut resp = t
      .http()
      .get("/api/packages?query=foo12")
      .call()
      .await
      .unwrap();
    let packages: ApiList<ApiPackage> = resp.expect_ok().await;
    assert_eq!(packages.items.len(), 11);

    let mut resp = t
      .http()
      .get(format!("/api/packages?query={}%20foo12", scope))
      .call()
      .await
      .unwrap();
    let packages: ApiList<ApiPackage> = resp.expect_ok().await;
    assert_eq!(packages.items.len(), 100);

    let url = format!("/api/packages?query=@{}&limit=10", t.scope.scope);
    // Check that search starting with `@` also works
    let mut resp = t.http().get(url).call().await.unwrap();
    let packages: ApiList<ApiPackage> = resp.expect_ok().await;
    assert_eq!(packages.items.len(), 10);

    let url = format!("/api/packages?query=@{}/foo&limit=15", t.scope.scope);
    // Check that search with `@scope/package` also works
    let mut resp = t.http().get(url).call().await.unwrap();
    let packages: ApiList<ApiPackage> = resp.expect_ok().await;
    assert_eq!(packages.items.len(), 15);

    let url = format!("/api/packages?gitHubRepoId={}", 5);
    // Check that search with `@scope/package` also works
    let mut resp = t.http().get(url).call().await.unwrap();
    let packages: ApiList<ApiPackage> = resp.expect_ok().await;
    assert_eq!(packages.items.len(), 15);
  }

  #[tokio::test]
  async fn test_packages_create() {
    let mut t = TestSetup::new().await;

    let mut resp = t
      .http()
      .post("/api/scopes/scope/packages")
      .body_json(json!({
        "package": "foo"
      }))
      .call()
      .await
      .unwrap();
    let package: ApiPackage = resp.expect_ok().await;
    assert_eq!(package.name, PackageName::try_from("foo").unwrap());

    let mut resp = t
      .http()
      .post("/api/scopes/scope/packages")
      .body_json(json!({
        "package": "foo"
      }))
      .call()
      .await
      .unwrap();
    resp
      .expect_err_code(StatusCode::CONFLICT, "packageAlreadyExists")
      .await;

    let mut resp = t
      .http()
      .post("/api/scopes/scope/packages")
      .body_json(json!({
        "package": "f-oo"
      }))
      .call()
      .await
      .unwrap();
    resp
      .expect_err_code(StatusCode::CONFLICT, "packageAlreadyExists")
      .await;

    let mut resp = t
      .http()
      .post("/api/scopes/scope2/packages")
      .body_json(json!({
        "package": "foo"
      }))
      .call()
      .await
      .unwrap();
    resp
      .expect_err_code(StatusCode::FORBIDDEN, "actorNotScopeMember")
      .await;

    let mut resp = t
      .http()
      .post("/api/scopes/scope/packages")
      .body_json(json!({
        "package": "somebadword"
      }))
      .call()
      .await
      .unwrap();
    resp
      .expect_err_code(StatusCode::BAD_REQUEST, "packageNameNotAllowed")
      .await;

    // create scope2 for user2, try creating a package with user1
    let scope2 = ScopeName::new("scope2".into()).unwrap();
    t.db().create_scope(&scope2, t.user2.user.id).await.unwrap();
    let mut resp = t
      .http()
      .post("/api/scopes/scope2/packages")
      .body_json(json!({
        "package": "foo"
      }))
      .call()
      .await
      .unwrap();
    resp
      .expect_err_code(StatusCode::FORBIDDEN, "actorNotScopeMember")
      .await;
  }

  #[tokio::test]
  async fn test_packages_get() {
    let mut t = TestSetup::new().await;

    let scope = t.scope.scope.clone();

    let name = PackageName::try_from("foo").unwrap();
    let res = t
      .ephemeral_database
      .create_package(&scope, &name)
      .await
      .unwrap();
    assert!(matches!(res, CreatePackageResult::Ok(_)));

    let mut resp = t
      .http()
      .get("/api/scopes/scope/packages/foo")
      .call()
      .await
      .unwrap();
    let package: ApiPackage = resp.expect_ok().await;
    assert_eq!(package.name, name);

    let mut resp = t
      .http()
      .get("/api/scopes/scope/packages/foo2")
      .call()
      .await
      .unwrap();
    resp
      .expect_err_code(StatusCode::NOT_FOUND, "packageNotFound")
      .await;

    let mut resp = t
      .http()
      .get("/api/scopes/scope2/packages/foo")
      .call()
      .await
      .unwrap();
    resp
      .expect_err_code(StatusCode::NOT_FOUND, "packageNotFound")
      .await;

    let mut resp = t
      .http()
      .get("/api/scopes/scope__asd/packages/foo")
      .call()
      .await
      .unwrap();
    resp
      .expect_err_code(StatusCode::BAD_REQUEST, "malformedRequest")
      .await;
  }

  #[tokio::test]
  async fn test_package_versions_list() {
    let mut t = TestSetup::new().await;

    let scope = t.scope.scope.clone();

    let name = PackageName::try_from("foo").unwrap();
    let res = t
      .ephemeral_database
      .create_package(&scope, &name)
      .await
      .unwrap();
    assert!(matches!(res, CreatePackageResult::Ok(_)));

    let mut resp = t
      .http()
      .get("/api/scopes/scope/packages/foo/versions")
      .call()
      .await
      .unwrap();
    let versions: Vec<ApiPackageVersion> = resp.expect_ok().await;
    assert!(versions.is_empty());

    t.ephemeral_database
      .create_package_version_for_test(NewPackageVersion {
        scope: &scope,
        name: &name,
        version: &"1.0.0".try_into().unwrap(),
        user_id: None,
        readme_path: None,
        uses_npm: false,
        exports: &ExportsMap::mock(),
        meta: Default::default(),
      })
      .await
      .unwrap();

    let mut resp = t
      .http()
      .get("/api/scopes/scope/packages/foo/versions")
      .call()
      .await
      .unwrap();
    let versions: Vec<ApiPackageVersion> = resp.expect_ok().await;
    assert_eq!(versions.len(), 1);
    assert_eq!(versions[0].version.to_string(), "1.0.0");

    let mut resp = t
      .http()
      .get("/api/scopes/scope/packages/foo2/versions")
      .call()
      .await
      .unwrap();
    resp
      .expect_err_code(StatusCode::NOT_FOUND, "packageNotFound")
      .await;
  }

  #[tokio::test]
  async fn test_package_version() {
    let mut t = TestSetup::new().await;

    let scope = t.scope.scope.clone();
    let name = PackageName::try_from("foo").unwrap();
    let res = t
      .ephemeral_database
      .create_package(&scope, &name)
      .await
      .unwrap();
    assert!(matches!(res, CreatePackageResult::Ok(_)));

    let mut resp = t
      .http()
      .get("/api/scopes/scope/packages/foo/versions/1.2.3")
      .call()
      .await
      .unwrap();
    resp
      .expect_err_code(StatusCode::NOT_FOUND, "packageVersionNotFound")
      .await;

    let version = Version::new("1.2.3").unwrap();
    let res = t
      .ephemeral_database
      .create_package_version_for_test(NewPackageVersion {
        scope: &scope,
        name: &name,
        version: &version,
        user_id: None,
        readme_path: None,
        uses_npm: false,
        exports: &ExportsMap::mock(),
        meta: Default::default(),
      })
      .await
      .unwrap();

    let mut resp = t
      .http()
      .get("/api/scopes/scope/packages/foo/versions/1.2.3")
      .call()
      .await
      .unwrap();
    let version: ApiPackageVersion = resp.expect_ok().await;
    assert_eq!(version.version, res.version);
    assert_eq!(version.uses_npm, res.uses_npm);
  }

  #[tokio::test]
  async fn test_package_provenance() {
    use crate::provenance::*;
    use base64::prelude::BASE64_STANDARD;
    use base64::Engine;

    let mut t = TestSetup::new().await;
    let scope = t.scope.scope.clone();

    let name = PackageName::try_from("foo").unwrap();
    let res = t
      .ephemeral_database
      .create_package(&scope, &name)
      .await
      .unwrap();
    assert!(matches!(res, CreatePackageResult::Ok(_)));

    t.ephemeral_database
      .create_package_version_for_test(NewPackageVersion {
        scope: &scope,
        name: &name,
        version: &"1.0.0".try_into().unwrap(),
        user_id: None,
        readme_path: None,
        uses_npm: false,
        exports: &ExportsMap::mock(),
        meta: Default::default(),
      })
      .await
      .unwrap();

    fn update_bundle_subject(bundle: &mut ProvenanceBundle, subject: Subject) {
      let subject = serde_json::json!({ "subject": [subject] });
      bundle.content.dsse_envelope.payload = BASE64_STANDARD
        .encode(serde_json::to_string(&subject).unwrap().as_bytes());
    }

    let mut bundle = ProvenanceBundle {
      media_type: "application/vnd.dsse.envelope.v1+json".to_string(),
      content: SignatureBundle {
        case: "dsseEnvelope".to_string(),
        dsse_envelope: Envelope {
          payload_type: "application/vnd.dsse.payload.v1+json".to_string(),
          payload: String::new(),
          signatures: [Signature {
            keyid: "keyid".to_string(),
            sig: "sig".to_string(),
          }],
        },
      },
      verification_material: VerificationMaterial {
        content: VerificationMaterialContent {
          case: "x509CertificateChain".to_string(),
          x509_certificate_chain: X509CertificateChain {
            certificates: [X509Certificate {
              raw_bytes: r#"-----BEGIN CERTIFICATE-----
MIIG2zCCBmGgAwIBAgIUdUUoLhVrbR1wlbMHQMvIvVNdv2swCgYIKoZIzj0EAwMw
NzEVMBMGA1UEChMMc2lnc3RvcmUuZGV2MR4wHAYDVQQDExVzaWdzdG9yZS1pbnRl
cm1lZGlhdGUwHhcNMjQwMjIzMTIxNjEyWhcNMjQwMjIzMTIyNjEyWjAAMFkwEwYH
KoZIzj0CAQYIKoZIzj0DAQcDQgAE3fcivk8ZWrFj83WJkVyWDQnpoTqQufcdFhVC
fbLmZ0Og/a5hIpDRM6QOsXeYb/esLET04MUJ9uov7T9IlkNqoaOCBYAwggV8MA4G
A1UdDwEB/wQEAwIHgDATBgNVHSUEDDAKBggrBgEFBQcDAzAdBgNVHQ4EFgQUrTqv
YqpMAh8yT1xr/zT5/0LVdEQwHwYDVR0jBBgwFoAU39Ppz1YkEZb5qNjpKFWixi4Y
ZD8waQYDVR0RAQH/BF8wXYZbaHR0cHM6Ly9naXRodWIuY29tL2xpdHRsZWRpdnkv
dGVzdF9wcm92ZW5hbmNlLy5naXRodWIvd29ya2Zsb3dzL3B1Ymxpc2gueW1sQHJl
ZnMvaGVhZHMvbWFpbjA5BgorBgEEAYO/MAEBBCtodHRwczovL3Rva2VuLmFjdGlv
bnMuZ2l0aHVidXNlcmNvbnRlbnQuY29tMBIGCisGAQQBg78wAQIEBHB1c2gwNgYK
KwYBBAGDvzABAwQoOTA2ZTQxNzljNzAxOTZiODE3OGNkZmYxMTdiZDMxYmUyOWJi
YjViZjAVBgorBgEEAYO/MAEEBAdQdWJsaXNoMCgGCisGAQQBg78wAQUEGmxpdHRs
ZWRpdnkvdGVzdF9wcm92ZW5hbmNlMB0GCisGAQQBg78wAQYED3JlZnMvaGVhZHMv
bWFpbjA7BgorBgEEAYO/MAEIBC0MK2h0dHBzOi8vdG9rZW4uYWN0aW9ucy5naXRo
dWJ1c2VyY29udGVudC5jb20wawYKKwYBBAGDvzABCQRdDFtodHRwczovL2dpdGh1
Yi5jb20vbGl0dGxlZGl2eS90ZXN0X3Byb3ZlbmFuY2UvLmdpdGh1Yi93b3JrZmxv
d3MvcHVibGlzaC55bWxAcmVmcy9oZWFkcy9tYWluMDgGCisGAQQBg78wAQoEKgwo
OTA2ZTQxNzljNzAxOTZiODE3OGNkZmYxMTdiZDMxYmUyOWJiYjViZjAdBgorBgEE
AYO/MAELBA8MDWdpdGh1Yi1ob3N0ZWQwPQYKKwYBBAGDvzABDAQvDC1odHRwczov
L2dpdGh1Yi5jb20vbGl0dGxlZGl2eS90ZXN0X3Byb3ZlbmFuY2UwOAYKKwYBBAGD
vzABDQQqDCg5MDZlNDE3OWM3MDE5NmI4MTc4Y2RmZjExN2JkMzFiZTI5YmJiNWJm
MB8GCisGAQQBg78wAQ4EEQwPcmVmcy9oZWFkcy9tYWluMBkGCisGAQQBg78wAQ8E
CwwJNzYyMjEzNzMxMC0GCisGAQQBg78wARAEHwwdaHR0cHM6Ly9naXRodWIuY29t
L2xpdHRsZWRpdnkwGAYKKwYBBAGDvzABEQQKDAgzNDk5NzY2NzBrBgorBgEEAYO/
MAESBF0MW2h0dHBzOi8vZ2l0aHViLmNvbS9saXR0bGVkaXZ5L3Rlc3RfcHJvdmVu
YW5jZS8uZ2l0aHViL3dvcmtmbG93cy9wdWJsaXNoLnltbEByZWZzL2hlYWRzL21h
aW4wOAYKKwYBBAGDvzABEwQqDCg5MDZlNDE3OWM3MDE5NmI4MTc4Y2RmZjExN2Jk
MzFiZTI5YmJiNWJmMBQGCisGAQQBg78wARQEBgwEcHVzaDBgBgorBgEEAYO/MAEV
BFIMUGh0dHBzOi8vZ2l0aHViLmNvbS9saXR0bGVkaXZ5L3Rlc3RfcHJvdmVuYW5j
ZS9hY3Rpb25zL3J1bnMvODAxOTAzMzc5NC9hdHRlbXB0cy8xMBYGCisGAQQBg78w
ARYECAwGcHVibGljMIGKBgorBgEEAdZ5AgQCBHwEegB4AHYA3T0wasbHETJjGR4c
mWc3AqJKXrjePK3/h4pygC8p7o4AAAGN1eUDuwAABAMARzBFAiB6G5EBxgUPYaoW
SU1hUeEAZOnqokMY57t4jIrt7zJ80gIhAP7z1Zvk6VSnYFN1WR5qXfuoKap9dRhk
kAguMO4NhRiAMAoGCCqGSM49BAMDA2gAMGUCMDwUicp4AAidIGwe4Ni8ySKAAm2u
rEeAxlF/0gbORF8pwLz5xbFlwYgy+fkN1pPZBgIxAItCHii6tGPOZOJlsjiR+dGr
ggHohNAjhbzDaY2iBW/m3NC5dehGUP4T2GBo/cwGhg==
-----END CERTIFICATE-----"#
                .to_string(),
            }],
          },
        },
        tlog_entries: [TlogEntry {
          log_index: 73446963,
        }],
      },
    };

    // Valid subject.
    update_bundle_subject(
      &mut bundle,
      Subject {
        name: format!("pkg:jsr/@{}/{}@1.0.0", scope, name),
        digest: SubjectDigest {
          sha256: "bar".to_string(),
        },
      },
    );
    let mut resp = t
      .http()
      .post("/api/scopes/scope/packages/foo/versions/1.0.0/provenance")
      .body_json(serde_json::json!({ "bundle": bundle }))
      .call()
      .await
      .unwrap();
    resp.expect_ok_no_content().await;

    let mut resp = t
      .http()
      .get("/api/scopes/scope/packages/foo/score")
      .call()
      .await
      .unwrap();
    let score: ApiPackageScore = resp.expect_ok().await;
    assert!(score.has_provenance);

    // Invalid subject.
    update_bundle_subject(
      &mut bundle,
      Subject {
        name: format!("pkg:jsr/@someotherscope/{}@1.0.0", name),
        digest: SubjectDigest {
          sha256: "baz".to_string(),
        },
      },
    );
    let mut resp = t
      .http()
      .post("/api/scopes/scope/packages/foo/versions/1.0.1/provenance")
      .body_json(serde_json::json!({ "bundle": bundle }))
      .call()
      .await
      .unwrap();
    resp
      .expect_err_code(StatusCode::INTERNAL_SERVER_ERROR, "internalServerError")
      .await;

    // Invalid certificate.
    bundle
      .verification_material
      .content
      .x509_certificate_chain
      .certificates[0]
      .raw_bytes = "invalid".to_string();
    let mut resp = t
      .http()
      .post("/api/scopes/scope/packages/foo/versions/1.0.1/provenance")
      .body_json(serde_json::json!({ "bundle": bundle }))
      .call()
      .await
      .unwrap();
    resp
      .expect_err_code(StatusCode::INTERNAL_SERVER_ERROR, "internalServerError")
      .await;
  }

  #[tokio::test]
  async fn test_package_latest_version() {
    let mut t = TestSetup::new().await;

    let scope = t.scope.scope.clone();

    let name = PackageName::try_from("foo").unwrap();
    let res = t
      .ephemeral_database
      .create_package(&scope, &name)
      .await
      .unwrap();
    assert!(matches!(res, CreatePackageResult::Ok(_)));

    t.ephemeral_database
      .create_package_version_for_test(NewPackageVersion {
        scope: &scope,
        name: &name,
        version: &"1.0.0".try_into().unwrap(),
        user_id: None,
        readme_path: None,
        uses_npm: false,
        exports: &ExportsMap::mock(),
        meta: Default::default(),
      })
      .await
      .unwrap();

    t.ephemeral_database
      .create_package_version_for_test(NewPackageVersion {
        scope: &scope,
        name: &name,
        version: &"1.0.1".try_into().unwrap(),
        user_id: None,
        readme_path: None,
        uses_npm: false,
        exports: &ExportsMap::mock(),
        meta: Default::default(),
      })
      .await
      .unwrap();

    t.ephemeral_database
      .create_package_version_for_test(NewPackageVersion {
        scope: &scope,
        name: &name,
        version: &"1.0.2-prerelease".try_into().unwrap(),
        user_id: None,
        readme_path: None,
        uses_npm: false,
        exports: &ExportsMap::mock(),
        meta: Default::default(),
      })
      .await
      .unwrap();

    let mut resp = t
      .http()
      .get("/api/scopes/scope/packages/foo")
      .call()
      .await
      .unwrap();
    let package: ApiPackage = resp.expect_ok().await;
    assert_eq!(package.latest_version.unwrap(), "1.0.1");
  }

  #[tokio::test]
  async fn update_package_description() {
    let mut t = TestSetup::new().await;

    let scope = t.scope.scope.clone();

    let name = PackageName::try_from("foo").unwrap();
    let res = t
      .ephemeral_database
      .create_package(&scope, &name)
      .await
      .unwrap();
    assert!(matches!(res, CreatePackageResult::Ok(_)));

    let mut resp = t
      .http()
      .get("/api/scopes/scope/packages/foo")
      .call()
      .await
      .unwrap();
    let package: ApiPackage = resp.expect_ok().await;
    assert_eq!(package.description, "");

    let mut resp = t
      .http()
      .patch("/api/scopes/scope/packages/foo")
      .body_json(json!({
        "description": "  foo \n  "
      }))
      .call()
      .await
      .unwrap();
    let package: ApiPackage = resp.expect_ok().await;
    assert_eq!(package.description, "foo");

    let mut resp = t
      .http()
      .patch("/api/scopes/scope/packages/foo")
      .body_json(json!({
        "description": "foo\nbar"
      }))
      .call()
      .await
      .unwrap();
    let package: ApiPackage = resp.expect_ok().await;
    assert_eq!(package.description, "foo bar");

    let mut resp = t
      .http()
      .patch("/api/scopes/scope/packages/foo")
      .body_json(json!({
        "description": "foo".repeat(100)
      }))
      .call()
      .await
      .unwrap();
    resp
      .expect_err_code(StatusCode::BAD_REQUEST, "malformedRequest")
      .await;

    let mut resp = t
      .http()
      .patch("/api/scopes/scope/packages/foo")
      .body_json(json!({
        "description": "foo\0baz"
      }))
      .call()
      .await
      .unwrap();
    resp
      .expect_err_code(StatusCode::BAD_REQUEST, "malformedRequest")
      .await;

    let mut resp = t
      .http()
      .get("/api/scopes/scope/packages/foo")
      .call()
      .await
      .unwrap();
    let package: ApiPackage = resp.expect_ok().await;
    assert_eq!(package.description, "foo bar");

    let mut resp = t
      .http()
      .patch("/api/scopes/scope/packages/foo2")
      .body_json(json!({
        "description": "bar"
      }))
      .call()
      .await
      .unwrap();
    resp
      .expect_err_code(StatusCode::NOT_FOUND, "packageNotFound")
      .await;
  }

  #[tokio::test]
  async fn update_package_runtime_compat() {
    let mut t = TestSetup::new().await;

    let scope = t.scope.scope.clone();

    let name = PackageName::try_from("foo").unwrap();
    let res = t
      .ephemeral_database
      .create_package(&scope, &name)
      .await
      .unwrap();
    assert!(matches!(res, CreatePackageResult::Ok(_)));

    let mut resp = t
      .http()
      .get("/api/scopes/scope/packages/foo")
      .call()
      .await
      .unwrap();
    let package: ApiPackage = resp.expect_ok().await;
    assert_eq!(package.runtime_compat.browser, None);
    assert_eq!(package.runtime_compat.deno, None);
    assert_eq!(package.runtime_compat.node, None);
    assert_eq!(package.runtime_compat.workerd, None);

    let mut resp = t
      .http()
      .patch("/api/scopes/scope/packages/foo")
      .body_json(json!({
        "runtimeCompat": {
          "deno": true
        }
      }))
      .call()
      .await
      .unwrap();
    let package: ApiPackage = resp.expect_ok().await;
    assert_eq!(package.runtime_compat.browser, None);
    assert_eq!(package.runtime_compat.deno, Some(true));
    assert_eq!(package.runtime_compat.node, None);
    assert_eq!(package.runtime_compat.workerd, None);

    let mut resp = t
      .http()
      .patch("/api/scopes/scope/packages/foo")
      .body_json(json!({
        "runtimeCompat": {
          "browser": true,
          "node": false
        }
      }))
      .call()
      .await
      .unwrap();
    let package: ApiPackage = resp.expect_ok().await;
    assert_eq!(package.runtime_compat.browser, Some(true));
    assert_eq!(package.runtime_compat.deno, None);
    assert_eq!(package.runtime_compat.node, Some(false));
    assert_eq!(package.runtime_compat.workerd, None);
    assert_eq!(package.runtime_compat.bun, None);

    let mut resp = t
      .http()
      .get("/api/scopes/scope/packages/foo")
      .call()
      .await
      .unwrap();
    let package: ApiPackage = resp.expect_ok().await;
    assert_eq!(package.runtime_compat.browser, Some(true));
    assert_eq!(package.runtime_compat.deno, None);
    assert_eq!(package.runtime_compat.node, Some(false));
    assert_eq!(package.runtime_compat.workerd, None);
    assert_eq!(package.runtime_compat.bun, None);

    let mut resp = t
      .http()
      .patch("/api/scopes/scope/packages/foo2")
      .body_json(json!({
        "runtimeCompat": {
          "browser": true,
          "node": false
        }
      }))
      .call()
      .await
      .unwrap();
    resp
      .expect_err_code(StatusCode::NOT_FOUND, "packageNotFound")
      .await;
  }

  #[tokio::test]
  async fn update_package_is_featured() {
    let mut t = TestSetup::new().await;

    let scope = t.scope.scope.clone();

    let name = PackageName::try_from("foo").unwrap();
    let res = t
      .ephemeral_database
      .create_package(&scope, &name)
      .await
      .unwrap();
    assert!(matches!(res, CreatePackageResult::Ok(_)));

    let mut resp = t
      .http()
      .get("/api/scopes/scope/packages/foo")
      .call()
      .await
      .unwrap();
    let package: ApiPackage = resp.expect_ok().await;
    assert!(package.when_featured.is_none());

    let mut resp = t
      .http()
      .patch("/api/scopes/scope/packages/foo")
      .body_json(json!({
        "isFeatured": true
      }))
      .call()
      .await
      .unwrap();
    // Non-staff users can make package featured.
    resp
      .expect_err_code(StatusCode::FORBIDDEN, "actorNotAuthorized")
      .await;

    let staff_token = t.staff_user.token.to_string();

    let mut resp = t
      .http()
      .patch("/api/scopes/scope/packages/foo")
      .body_json(json!({
        "isFeatured": true
      }))
      .token(Some(&staff_token))
      .call()
      .await
      .unwrap();
    let package: ApiPackage = resp.expect_ok().await;
    assert!(package.when_featured.is_some());

    let mut resp = t
      .http()
      .patch("/api/scopes/scope/packages/foo")
      .body_json(json!({
        "isFeatured": false
      }))
      .token(Some(&staff_token))
      .call()
      .await
      .unwrap();
    let package: ApiPackage = resp.expect_ok().await;
    assert!(package.when_featured.is_none());
  }

  #[tokio::test]
  async fn test_package_limit() {
    let t = TestSetup::new().await;

    let scope = t.scope.scope.clone();
    t.ephemeral_database
      .update_scope_limits(&t.scope.scope, Some(10), Some(100), Some(100))
      .await
      .unwrap();

    for i in 1..=11 {
      let name = PackageName::new(format!("foo{i}")).unwrap();
      let res = t.ephemeral_database.create_package(&scope, &name).await;

      if i < 11 {
        assert!(matches!(res.unwrap(), CreatePackageResult::Ok(_)));
      } else {
        assert!(matches!(
          res.unwrap(),
          CreatePackageResult::PackageLimitExceeded(10)
        ));
      }
    }
  }

  #[tokio::test]
  async fn test_package_weekly_limit() {
    let t = TestSetup::new().await;

    let scope = t.scope.scope.clone();
    t.ephemeral_database
      .update_scope_limits(&t.scope.scope, Some(100), Some(10), Some(100))
      .await
      .unwrap();

    for i in 1..=11 {
      let name = PackageName::new(format!("foo{i}")).unwrap();
      let res = t.ephemeral_database.create_package(&scope, &name).await;

      if i < 11 {
        assert!(matches!(res.unwrap(), CreatePackageResult::Ok(_)));
      } else {
        assert!(matches!(
          res.unwrap(),
          CreatePackageResult::WeeklyPackageLimitExceeded(10)
        ));
      }
    }
  }

  #[tokio::test]
  async fn test_publishing_attempts_weekly_limit() {
    let mut t = TestSetup::new().await;

    let scope = t.scope.scope.clone();
    t.ephemeral_database
      .update_scope_limits(&t.scope.scope, Some(100), Some(100), Some(10))
      .await
      .unwrap();

    let name = PackageName::new("foo".to_owned()).unwrap();
    let config_file = PackagePath::try_from("/jsr.json").unwrap();

    let CreatePackageResult::Ok(package) =
      t.db().create_package(&scope, &name).await.unwrap()
    else {
      unreachable!();
    };

    for i in 1..=10 {
      let res = t
        .db()
        .create_publishing_task(NewPublishingTask {
          package_scope: &scope,
          package_name: &package.name,
          package_version: &Version::new(&format!("0.0.{i}")).unwrap(),
          config_file: &config_file,
          user_id: None,
        })
        .await
        .unwrap();
      assert!(
        matches!(res, CreatePublishingTaskResult::Created(_)),
        "{res:?}",
      );
    }

    let data = create_mock_tarball("ok");
    let mut resp = t
      .http()
      .post("/api/scopes/scope/packages/foo/versions/1.2.3?config=/jsr.json")
      .gzip()
      .body(Body::from(data))
      .call()
      .await
      .unwrap();
    resp
      .expect_err_code(
        StatusCode::BAD_REQUEST,
        "weeklyPublishAttemptsLimitExceeded",
      )
      .await;
  }

  #[tokio::test]
  async fn test_publishing_with_missing_auth() {
    let mut t = TestSetup::new().await;

    let permission =
      Permission::PackagePublish(PackagePublishPermission::Scope {
        scope: ScopeName::new("otherscope".to_owned()).unwrap(),
      });

    let token = create_token(
      &t.db(),
      t.user1.user.id,
      TokenType::Web,
      None,
      None,
      Some(Permissions(vec![permission])),
    )
    .await
    .unwrap();

    let scope = t.scope.scope.clone();

    let name = PackageName::new("foo".to_owned()).unwrap();

    let CreatePackageResult::Ok(_) =
      t.db().create_package(&scope, &name).await.unwrap()
    else {
      unreachable!();
    };

    let data = create_mock_tarball("ok");
    let mut resp = t
      .http()
      .post("/api/scopes/scope/packages/foo/versions/1.2.3?config=/jsr.json")
      .gzip()
      .token(Some(&token))
      .body(Body::from(data))
      .call()
      .await
      .unwrap();
    resp
      .expect_err_code(StatusCode::FORBIDDEN, "missingPermission")
      .await;
  }

  #[tokio::test]
  async fn test_package_docs() {
    let mut t = TestSetup::new().await;

    // unpublished package
    let mut resp = t
      .http()
      .get("/api/scopes/scope/packages/foo/versions/0.0.1/docs")
      .call()
      .await
      .unwrap();
    resp
      .expect_err_code(StatusCode::NOT_FOUND, "packageNotFound")
      .await;

    let task = process_tarball_setup(&t, create_mock_tarball("ok")).await;
    assert_eq!(task.status, PublishingTaskStatus::Success, "{:?}", task);

    // index page
    let mut resp = t
      .http()
      .get("/api/scopes/scope/packages/foo/versions/1.2.3/docs")
      .call()
      .await
      .unwrap();
    let docs: ApiPackageVersionDocs = resp.expect_ok().await;
    match docs {
      ApiPackageVersionDocs::Content {
        version,
        css,
        comrak_css: _,
        script: _,
        breadcrumbs,
        toc,
        main: _,
      } => {
        assert_eq!(version.version, task.package_version);
        assert!(css.contains("{max-width:"), "{}", css);
        assert!(breadcrumbs.is_none(), "{:?}", breadcrumbs);
        assert!(toc.is_some(), "{:?}", toc)
      }
      ApiPackageVersionDocs::Redirect { .. } => panic!(),
    }

    // all_symbols page
    let mut resp = t
      .http()
      .get("/api/scopes/scope/packages/foo/versions/1.2.3/docs?all_symbols")
      .call()
      .await
      .unwrap();
    let docs: ApiPackageVersionDocs = resp.expect_ok().await;
    match docs {
      ApiPackageVersionDocs::Content {
        version,
        css,
        comrak_css: _,
        script: _,
        breadcrumbs,
        toc,
        main: _,
      } => {
        assert_eq!(version.version, task.package_version);
        assert!(css.contains("{max-width:"), "{}", css);
        assert!(
          breadcrumbs.as_ref().unwrap().contains("all symbols"),
          "{:?}",
          breadcrumbs
        );
        assert!(toc.is_none(), "{:?}", toc);
      }
      ApiPackageVersionDocs::Redirect { .. } => panic!(),
    }

    // symbol page
    let mut resp = t
      .http()
      .get("/api/scopes/scope/packages/foo/versions/1.2.3/docs?symbol=hello")
      .call()
      .await
      .unwrap();
    let docs: ApiPackageVersionDocs = resp.expect_ok().await;
    match docs {
      ApiPackageVersionDocs::Content {
        version,
        css,
        comrak_css: _,
        script: _,
        breadcrumbs,
        toc,
        main: _,
      } => {
        assert_eq!(version.version, task.package_version);
        assert!(css.contains("{max-width:"), "{}", css);
        assert!(
          breadcrumbs.as_ref().unwrap().contains("hello"),
          "{:?}",
          breadcrumbs
        );
        assert!(toc.is_some(), "{:?}", toc);
      }
      ApiPackageVersionDocs::Redirect { .. } => panic!(),
    }

    // symbol page
    let mut resp = t
      .http()
      .get(format!(
        "/api/scopes/scope/packages/foo/versions/1.2.3/docs?symbol={}",
        urlencoding::encode("读取多键1")
      ))
      .call()
      .await
      .unwrap();
    let docs: ApiPackageVersionDocs = resp.expect_ok().await;
    match docs {
      ApiPackageVersionDocs::Content {
        version,
        css,
        comrak_css: _,
        script: _,
        breadcrumbs,
        toc,
        main: _,
      } => {
        assert_eq!(version.version, task.package_version);
        assert!(css.contains("{max-width:"), "{}", css);
        assert!(
          breadcrumbs.as_ref().unwrap().contains("读取多键1"),
          "{:?}",
          breadcrumbs
        );
        assert!(toc.is_some(), "{:?}", toc);
      }
      ApiPackageVersionDocs::Redirect { .. } => panic!(),
    }

    // search
    let mut resp = t
      .http()
      .get("/api/scopes/scope/packages/foo/versions/1.2.3/docs/search")
      .call()
      .await
      .unwrap();
    let search: serde_json::Value = resp.expect_ok().await;
    assert_eq!(
      search,
      json!({"nodes":[{"kind":[{"kind":"Variable","char":"v","title":"Variable"}],"name":"hello","file":".","doc":"This is a test constant.","url":"/@scope/foo@1.2.3/doc/~/hello","deprecated":false},{"kind":[{"kind":"Variable","char":"v","title":"Variable"}],"name":"读取多键1","file":".","doc":"","url":"/@scope/foo@1.2.3/doc/~/读取多键1","deprecated":false}]}),
    );

    // symbol doesn't exist
    let mut resp = t
      .http()
      .get("/api/scopes/scope/packages/foo/versions/1.2.3/docs?symbol=asdf")
      .call()
      .await
      .unwrap();
    resp
      .expect_err_code(StatusCode::NOT_FOUND, "entrypointOrSymbolNotFound")
      .await;

    // entrypoint doesn't exist
    let mut resp = t
      .http()
      .get("/api/scopes/scope/packages/foo/versions/1.2.3/docs?entrypoint=asdf")
      .call()
      .await
      .unwrap();
    resp
      .expect_err_code(StatusCode::NOT_FOUND, "entrypointOrSymbolNotFound")
      .await;
  }

  #[tokio::test]
  async fn test_package_dependencies_and_dependents() {
    let mut t = TestSetup::new().await;

    // unpublished package
    let mut resp = t
      .http()
      .get("/api/scopes/scope/packages/foo/versions/0.0.1/dependencies")
      .call()
      .await
      .unwrap();
    resp
      .expect_err_code(StatusCode::NOT_FOUND, "packageVersionNotFound")
      .await;
    let mut resp = t
      .http()
      .get("/api/scopes/scope/packages/foo/dependents")
      .call()
      .await
      .unwrap();
    resp
      .expect_err_code(StatusCode::NOT_FOUND, "packageNotFound")
      .await;

    let task = process_tarball_setup(&t, create_mock_tarball("ok")).await;
    assert_eq!(task.status, PublishingTaskStatus::Success, "{:?}", task);

    // Empty deps
    let mut resp = t
      .http()
      .get("/api/scopes/scope/packages/foo/versions/1.2.3/dependencies")
      .call()
      .await
      .unwrap();
    let deps: Vec<ApiDependency> = resp.expect_ok().await;
    assert_eq!(deps.len(), 0);
    let mut resp = t
      .http()
      .get("/api/scopes/scope/packages/foo/dependents")
      .call()
      .await
      .unwrap();
    let dependents: ApiList<ApiDependent> = resp.expect_ok().await;
    assert_eq!(dependents.items.len(), 0);

    // Now publish a package that has a few deps

    let package_name = PackageName::try_from("bar").unwrap();
    let version = Version::try_from("1.2.3").unwrap();
    let task = crate::publish::tests::process_tarball_setup2(
      &t,
      create_mock_tarball("depends_on_ok"),
      &package_name,
      &version,
      false,
    )
    .await;
    assert_eq!(task.status, PublishingTaskStatus::Success, "{:?}", task);

    let mut resp = t
      .http()
      .get("/api/scopes/scope/packages/bar/versions/1.2.3/dependencies")
      .call()
      .await
      .unwrap();
    let deps: Vec<ApiDependency> = resp.expect_ok().await;
    assert_eq!(
      deps,
      vec![
        ApiDependency {
          kind: ApiDependencyKind::Jsr,
          name: "@scope/foo".to_string(),
          constraint: "1".to_string(),
          path: "".to_string()
        },
        ApiDependency {
          kind: ApiDependencyKind::Npm,
          name: "express".to_string(),
          constraint: "4".to_string(),
          path: "".to_string()
        },
      ],
    );

    let mut resp = t
      .http()
      .get("/api/scopes/scope/packages/foo/dependents")
      .call()
      .await
      .unwrap();
    let dependents: ApiList<ApiDependent> = resp.expect_ok().await;
    assert_eq!(
      &dependents.items,
      &[ApiDependent {
        scope: "scope".try_into().unwrap(),
        package: "bar".try_into().unwrap(),
        versions: vec!["1.2.3".try_into().unwrap()],
        total_versions: 1,
      }]
    );

    let package_name = PackageName::try_from("bar").unwrap();
    let version = Version::try_from("1.2.4").unwrap();
    let task = crate::publish::tests::process_tarball_setup2(
      &t,
      create_mock_tarball("depends_on_ok2"),
      &package_name,
      &version,
      false,
    )
    .await;
    assert_eq!(task.status, PublishingTaskStatus::Success, "{:?}", task);

    let package_name = PackageName::try_from("baz").unwrap();
    let version = Version::try_from("1.2.3").unwrap();
    let task = crate::publish::tests::process_tarball_setup2(
      &t,
      create_mock_tarball("depends_on_ok3"),
      &package_name,
      &version,
      false,
    )
    .await;
    assert_eq!(task.status, PublishingTaskStatus::Success, "{:?}", task);

    let mut resp = t
      .http()
      .get("/api/scopes/scope/packages/foo/dependents")
      .call()
      .await
      .unwrap();
    let dependents: ApiList<ApiDependent> = resp.expect_ok().await;
    assert_eq!(
      &dependents.items,
      &[
        ApiDependent {
          scope: "scope".try_into().unwrap(),
          package: "bar".try_into().unwrap(),
          versions: vec![
            "1.2.3".try_into().unwrap(),
            "1.2.4".try_into().unwrap()
          ],
          total_versions: 2,
        },
        ApiDependent {
          scope: "scope".try_into().unwrap(),
          package: "baz".try_into().unwrap(),
          versions: vec!["1.2.3".try_into().unwrap()],
          total_versions: 1,
        }
      ]
    );
    assert_eq!(dependents.total, 2);

    let mut resp = t
      .http()
      .get("/api/scopes/scope/packages/foo/dependents?limit=1")
      .call()
      .await
      .unwrap();
    let dependents: ApiList<ApiDependent> = resp.expect_ok().await;
    assert_eq!(
      dependents.items,
      vec![ApiDependent {
        scope: "scope".try_into().unwrap(),
        package: "bar".try_into().unwrap(),
        versions: vec![
          "1.2.3".try_into().unwrap(),
          "1.2.4".try_into().unwrap()
        ],
        total_versions: 2,
      }]
    );
    assert_eq!(dependents.total, 2);

    let mut resp = t
      .http()
      .get("/api/scopes/scope/packages/foo/dependents?limit=1&page=2")
      .call()
      .await
      .unwrap();
    let dependents: ApiList<ApiDependent> = resp.expect_ok().await;
    assert_eq!(
      dependents.items,
      vec![ApiDependent {
        scope: "scope".try_into().unwrap(),
        package: "baz".try_into().unwrap(),
        versions: vec!["1.2.3".try_into().unwrap()],
        total_versions: 1,
      }]
    );
    assert_eq!(dependents.total, 2);

    let mut resp = t
      .http()
      .get("/api/scopes/scope/packages/foo/dependents?versions_per_package_limit=1")
      .call()
      .await
      .unwrap();
    let dependents: ApiList<ApiDependent> = resp.expect_ok().await;
    assert_eq!(
      dependents.items,
      vec![
        ApiDependent {
          scope: "scope".try_into().unwrap(),
          package: "bar".try_into().unwrap(),
          versions: vec!["1.2.3".try_into().unwrap(),],
          total_versions: 2,
        },
        ApiDependent {
          scope: "scope".try_into().unwrap(),
          package: "baz".try_into().unwrap(),
          versions: vec!["1.2.3".try_into().unwrap()],
          total_versions: 1,
        },
      ],
    );
    assert_eq!(dependents.total, 2);
  }

  #[tokio::test]
  async fn package_delete() {
    let mut t: TestSetup = TestSetup::new().await;

    let scope = t.scope.scope.clone();
    let name = PackageName::try_from("foo").unwrap();

    let res = t
      .ephemeral_database
      .create_package(&scope, &name)
      .await
      .unwrap();
    assert!(matches!(res, CreatePackageResult::Ok(_)));

    let url = format!("/api/scopes/{}/packages/{}", scope, name);
    let mut resp = t.http().delete(url).call().await.unwrap();
    resp.expect_ok_no_content().await;
  }

  #[tokio::test]
  async fn package_delete_not_admin() {
    let mut t: TestSetup = TestSetup::new().await;

    let scope = t.scope.scope.clone();
    t.db()
      .add_scope_invite(NewScopeInvite {
        target_user_id: t.user2.user.id,
        requesting_user_id: t.user1.user.id,
        scope: &scope,
      })
      .await
      .unwrap();
    t.db()
      .accept_scope_invite(&t.user2.user.id, &scope)
      .await
      .unwrap();

    let name = PackageName::try_from("foo").unwrap();

    let res = t
      .ephemeral_database
      .create_package(&scope, &name)
      .await
      .unwrap();
    assert!(matches!(res, CreatePackageResult::Ok(_)));

    let url = format!("/api/scopes/{}/packages/{}", scope, name);
    let token = t.user2.token.clone();
    let mut resp = t
      .http()
      .delete(url)
      .token(Some(&token))
      .call()
      .await
      .unwrap();
    resp
      .expect_err_code(StatusCode::FORBIDDEN, "actorNotScopeAdmin")
      .await;
  }

  #[tokio::test]
  async fn package_delete_not_member() {
    let mut t: TestSetup = TestSetup::new().await;

    let scope = t.scope.scope.clone();
    let name = PackageName::try_from("foo").unwrap();

    let res = t
      .ephemeral_database
      .create_package(&scope, &name)
      .await
      .unwrap();
    assert!(matches!(res, CreatePackageResult::Ok(_)));

    let url = format!("/api/scopes/{}/packages/{}", scope, name);
    let token = t.user3.token.clone();
    let mut resp = t
      .http()
      .delete(url)
      .token(Some(&token))
      .call()
      .await
      .unwrap();
    resp
      .expect_err_code(StatusCode::FORBIDDEN, "actorNotScopeMember")
      .await;
  }

  #[tokio::test]
  async fn package_delete_not_empty() {
    let mut t: TestSetup = TestSetup::new().await;

    let scope = t.scope.scope.clone();
    let name = PackageName::try_from("foo").unwrap();

    let version = Version::try_from("1.2.3").unwrap();
    let task = crate::publish::tests::process_tarball_setup2(
      &t,
      create_mock_tarball("ok"),
      &name,
      &version,
      false,
    )
    .await;
    assert_eq!(task.status, PublishingTaskStatus::Success, "{:?}", task);

    let url = format!("/api/scopes/{}/packages/{}", scope, name);
    let mut resp = t.http().delete(url).call().await.unwrap();
    resp
      .expect_err_code(StatusCode::CONFLICT, "packageNotEmpty")
      .await;
  }

  #[tokio::test]
  async fn package_delete_is_publishing() {
    let mut t: TestSetup = TestSetup::new().await;

    let scope = t.scope.scope.clone();
    let name = PackageName::try_from("foo").unwrap();

    let res = t
      .ephemeral_database
      .create_package(&scope, &name)
      .await
      .unwrap();
    assert!(matches!(res, CreatePackageResult::Ok(_)));

    let version = Version::try_from("1.2.3").unwrap();
    let config_file = PackagePath::try_from("/jsr.json").unwrap();
    t.db()
      .create_publishing_task(NewPublishingTask {
        user_id: Some(t.user1.user.id),
        package_scope: &scope,
        package_name: &name,
        package_version: &version,
        config_file: &config_file,
      })
      .await
      .unwrap();

    let url = format!("/api/scopes/{}/packages/{}", scope, name);
    let mut resp = t.http().delete(url).call().await.unwrap();
    resp
      .expect_err_code(StatusCode::CONFLICT, "packageNotEmpty")
      .await;
  }

  #[tokio::test]
  async fn archive_package() {
    let mut t = TestSetup::new().await;

    let scope = t.scope.scope.clone();

    let name = PackageName::try_from("foo").unwrap();
    let res = t
      .ephemeral_database
      .create_package(&scope, &name)
      .await
      .unwrap();
    assert!(matches!(res, CreatePackageResult::Ok(_)));

    let mut resp = t
      .http()
      .get("/api/scopes/scope/packages/foo")
      .call()
      .await
      .unwrap();
    let package: ApiPackage = resp.expect_ok().await;
    assert!(!package.is_archived);

    let mut resp = t
      .http()
      .patch("/api/scopes/scope/packages/foo")
      .body_json(json!({
        "isArchived": true
      }))
      .call()
      .await
      .unwrap();
    let package: ApiPackage = resp.expect_ok().await;
    assert!(package.is_archived);

    let mut resp = t
      .http()
      .patch("/api/scopes/scope/packages/foo")
      .body_json(json!({
        "description": "foo"
      }))
      .call()
      .await
      .unwrap();
    resp
      .expect_err_code(StatusCode::BAD_REQUEST, "packageArchived")
      .await;

    let data = create_mock_tarball("ok");
    let mut resp = t
      .http()
      .post("/api/scopes/scope/packages/foo/versions/1.2.3?config=/jsr.json")
      .gzip()
      .body(Body::from(data))
      .call()
      .await
      .unwrap();
    resp
      .expect_err_code(StatusCode::BAD_REQUEST, "packageArchived")
      .await;

    let mut resp = t
      .http()
      .patch("/api/scopes/scope/packages/foo")
      .body_json(json!({
        "isArchived": false
      }))
      .call()
      .await
      .unwrap();
    let package: ApiPackage = resp.expect_ok().await;
    assert!(!package.is_archived);
  }

  #[tokio::test]
  async fn package_source() {
    let mut t: TestSetup = TestSetup::new().await;

    let task = process_tarball_setup(&t, create_mock_tarball("deep")).await;
    assert_eq!(task.status, PublishingTaskStatus::Success, "{:?}", task);

    let url = format!(
      "/api/scopes/{}/packages/{}/versions/{}/source?path=/",
      task.package_scope, task.package_name, task.package_version
    );
    let mut resp = t.http().get(url).call().await.unwrap();
    let body = resp.expect_ok::<ApiPackageVersionSource>().await;

    let ApiSource::Dir { entries } = body.source else {
      panic!();
    };

    assert_eq!(
      entries,
      vec![
        ApiSourceDirEntry {
          name: "a".to_string(),
          size: 29,
          kind: ApiSourceDirEntryKind::Dir,
        },
        ApiSourceDirEntry {
          name: "b".to_string(),
          size: 14,
          kind: ApiSourceDirEntryKind::Dir,
        },
        ApiSourceDirEntry {
          name: "bin.bin".to_string(),
          size: 1000,
          kind: ApiSourceDirEntryKind::File,
        },
        ApiSourceDirEntry {
          name: "jsr.json".to_string(),
          size: 74,
          kind: ApiSourceDirEntryKind::File,
        },
        ApiSourceDirEntry {
          name: "mod.ts".to_string(),
          size: 124,
          kind: ApiSourceDirEntryKind::File,
        }
      ]
    );

    let url = format!(
      "/api/scopes/{}/packages/{}/versions/{}/source?path=/mod.ts",
      task.package_scope, task.package_name, task.package_version
    );
    let mut resp = t.http().get(url).call().await.unwrap();
    let body = resp.expect_ok::<ApiPackageVersionSource>().await;

    let ApiSource::File { size, view } = body.source else {
      panic!();
    };

    assert_eq!(size, 124);
    assert!(view.is_some());

    let url = format!(
      "/api/scopes/{}/packages/{}/versions/{}/source?path=/bin.bin",
      task.package_scope, task.package_name, task.package_version
    );
    let mut resp = t.http().get(url).call().await.unwrap();
    let body = resp.expect_ok::<ApiPackageVersionSource>().await;

    let ApiSource::File { size, view } = body.source else {
      panic!();
    };

    assert_eq!(size, 1000);
    assert!(view.is_none());
  }

  #[tokio::test]
  async fn metrics() {
    let mut t: TestSetup = TestSetup::new().await;
    let mut resp = t.unauthed_http().get("/api/metrics").call().await.unwrap();
    let body = resp.expect_ok::<ApiMetrics>().await;

    assert_eq!(0, body.packages);
    assert_eq!(0, body.packages_1d);
    assert_eq!(0, body.packages_7d);
    assert_eq!(0, body.packages_30d);

    assert_eq!(5, body.users);
    assert_eq!(5, body.users_1d);
    assert_eq!(5, body.users_7d);
    assert_eq!(5, body.users_30d);

    assert_eq!(0, body.package_versions);
    assert_eq!(0, body.package_versions_1d);
    assert_eq!(0, body.package_versions_7d);
    assert_eq!(0, body.package_versions_30d);
  }

  #[tokio::test]
  async fn publishing_tasks_list() {
    let mut t = TestSetup::new().await;
    let scope_name = ScopeName::try_from("scope").unwrap();
    let package_name = PackageName::try_from("foo").unwrap();
    let version = Version::try_from("1.2.3").unwrap();
    let task = process_tarball_setup2(
      &t,
      create_mock_tarball("name_mismatch"),
      &package_name,
      &version,
      false,
    )
    .await;
    assert_eq!(task.status, PublishingTaskStatus::Failure);

    let tasks = t
      .ephemeral_database
      .list_publishing_tasks_for_package(&scope_name, &package_name)
      .await
      .unwrap();
    assert_eq!(tasks.len(), 1);

    t.http()
      .delete(&format!(
        "/api/scopes/{}/packages/{}",
        scope_name, package_name
      ))
      .call()
      .await
      .unwrap();

    t.db()
      .create_package(&scope_name, &package_name)
      .await
      .unwrap();

    let task2 = process_tarball_setup2(
      &t,
      create_mock_tarball("ok"),
      &package_name,
      &version,
      false,
    )
    .await;
    assert_eq!(task2.status, PublishingTaskStatus::Success);

    let tasks = t
      .ephemeral_database
      .list_publishing_tasks_for_package(&scope_name, &package_name)
      .await
      .unwrap();
    assert_eq!(tasks.len(), 1);
    assert_eq!(tasks[0].id, task2.id);
  }
}
