// Copyright 2024 the JSR authors. All rights reserved. MIT license.
use futures::FutureExt;
use hyper::body;
use hyper::header;
use hyper::header::COOKIE;
use hyper::Body;
use hyper::Request;
use hyper::Response;
use hyper::StatusCode;
use oauth2::http::HeaderName;
use routerify::prelude::RequestExt;
use routerify_query::RequestQueryExt;
use serde::de::DeserializeOwned;
use serde::Serialize;
use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;
use tracing::error;
use tracing::field;
use tracing::instrument;
use tracing::Span;
use url::Url;
use uuid::Uuid;

use crate::api::ApiError;
use crate::db::Database;
use crate::db::Permissions;
use crate::github::verify_oidc_token;
use crate::iam::IamInfo;
use crate::iam::ReqIamExt as _;
use crate::ids::PackageName;
use crate::ids::ScopeName;
use crate::ids::Version;

pub const USER_AGENT: &str = "JSR";

pub type ApiResult<D> = Result<D, ApiError>;

pub type ApiHandlerFuture<D> =
  Pin<Box<dyn Future<Output = ApiResult<D>> + Send>>;

/// Wrap an endpoint handler converting it's success return value into a JSON response
///
/// Uses [`respond_json`] under the hood, with a 200 status code.
pub fn json<D, H, HF>(
  handler: H,
) -> impl Fn(Request<Body>) -> ApiHandlerFuture<Response<Body>>
where
  D: Serialize,
  H: Fn(Request<Body>) -> HF,
  HF: Future<Output = ApiResult<D>> + Send + 'static,
{
  move |req: Request<Body>| {
    let fut = handler(req);
    async move {
      let data = fut.await?;
      Ok::<Response<Body>, ApiError>(respond_json(&data, StatusCode::OK))
    }
    .boxed()
  }
}

pub fn respond_json<V: ?Sized + Serialize>(
  value: &V,
  status_code: StatusCode,
) -> Response<Body> {
  let body = serde_json::to_string(value).unwrap();
  create_response(status_code, "application/json", body)
}

pub fn create_response<B>(
  status: StatusCode,
  mime: &str, // TODO mime::Mime,
  body: B,
) -> Response<Body>
where
  B: Into<Body>,
{
  Response::builder()
    .status(status)
    .header(header::CONTENT_TYPE, mime)
    .body(body.into())
    .expect("expected to be able to create response")
}

pub fn auth<H, HF>(
  handler: H,
) -> impl Fn(Request<Body>) -> ApiHandlerFuture<Response<Body>>
where
  H: Send + Sync + Fn(Request<Body>) -> HF + Send + 'static,
  HF: Future<Output = ApiResult<Response<Body>>> + Send + 'static,
{
  let handler = Arc::new(handler);
  move |req: Request<Body>| {
    let handler = handler.clone();
    async move {
      if req.context::<IamInfo>().is_some() {
        let hf = handler(req);
        hf.await
      } else {
        Err(ApiError::MissingAuthentication)
      }
    }
    .map(|res| {
      if let Err(err) = &res {
        error!({ code = err.code() }, "{}", err.message());
      }
      res
    })
    .boxed()
  }
}

pub struct CacheDuration(pub usize);
impl CacheDuration {
  pub const ONE_MINUTE: CacheDuration = CacheDuration(60);
}

pub fn cache<H, HF>(
  duration: CacheDuration,
  handler: H,
) -> impl Fn(Request<Body>) -> ApiHandlerFuture<Response<Body>>
where
  H: Send + Sync + Fn(Request<Body>) -> HF + Send + 'static,
  HF: Future<Output = ApiResult<Response<Body>>> + Send + 'static,
{
  let value =
    header::HeaderValue::from_str(&format!("public, s-maxage={}", duration.0))
      .unwrap();
  let handler = Arc::new(handler);
  move |req: Request<Body>| {
    let handler = handler.clone();
    let value = value.clone();
    async move {
      let is_anonymous = req.iam().is_anonymous();
      let mut res = handler(req).await?;
      if is_anonymous {
        res
          .headers_mut()
          .entry(header::CACHE_CONTROL)
          .or_insert_with(|| value);
      }
      Ok(res)
    }
    .boxed()
  }
}

#[instrument(name = "auth", skip(req), err, fields(token.kind, user.id, repo.id))]
pub async fn auth_middleware(req: Request<Body>) -> ApiResult<Request<Body>> {
  let db = req.data::<Database>().unwrap();
  let token = extract_token_and_sudo(&req);

  let span = Span::current();

  let iam_info =
    match token {
      Some((AuthorizationToken::Bearer(token), sudo)) => {
        span.record("token.kind", &field::display("bearer"));
        if let Some(token) =
          db.get_token_by_hash(&crate::token::hash(token)).await?
        {
          if let Some(expires_at) = token.expires_at {
            if expires_at < chrono::Utc::now() {
              return Err(ApiError::InvalidBearerToken);
            }
          }

          let user = db.get_user(token.user_id).await?.unwrap();
          span.record("user.id", &field::display(user.id));

          if user.is_blocked {
            return Err(ApiError::Blocked);
          }

          IamInfo::from((token, user, sudo))
        } else {
          return Err(ApiError::InvalidBearerToken);
        }
      }
      Some((AuthorizationToken::GithubOIDC(token), _)) => {
        span.record("token.kind", &field::display("githuboidc"));

        let claims = verify_oidc_token(token).await?;
        span.record("repo.id", &field::display(claims.repository_id));

        let aud: GithubOidcTokenAud = serde_json::from_str(&claims.aud)
          .map_err(|err| ApiError::InvalidOidcToken {
            msg: format!("failed to parse 'aud': {err}").into(),
          })?;

        let user = db.get_user_by_github_id(claims.actor_id).await?;
        if let Some(user) = &user {
          span.record("user.id", &field::display(user.id));
        }

        IamInfo::from((claims.repository_id, aud, user))
      }
      None => IamInfo::anonymous(),
    };

  req.set_context(iam_info);

  Ok(req)
}

enum AuthorizationToken<'s> {
  Bearer(&'s str),
  GithubOIDC(&'s str),
}

static X_JSR_SUDO: HeaderName = header::HeaderName::from_static("x-jsr-sudo");

fn extract_token_and_sudo(
  req: &Request<Body>,
) -> Option<(AuthorizationToken, bool)> {
  let headers = req.headers();

  let mut sudo = headers
    .get(&X_JSR_SUDO)
    .map(|v| v == "true")
    .unwrap_or(false);

  for cookie in headers.get_all(COOKIE) {
    let mut return_val = Option::<AuthorizationToken>::None;
    if let Ok(cookie) = cookie.to_str() {
      for cookie in cookie.split(';') {
        if let Some(token) = cookie.trim().strip_prefix("token=") {
          return_val = Some(AuthorizationToken::Bearer(token));
        }
        if cookie.trim() == "sudo=1" {
          sudo = true;
        }
      }
    }
    if let Some(token) = return_val {
      return Some((token, sudo));
    }
  }

  if let Some(auth) = headers.get(header::AUTHORIZATION) {
    if let Ok(auth) = auth.to_str() {
      if let Some(token) = auth.strip_prefix("Bearer ") {
        return Some((AuthorizationToken::Bearer(token), sudo));
      }
      if let Some(token) = auth.strip_prefix("githuboidc ") {
        return Some((AuthorizationToken::GithubOIDC(token), sudo));
      }
    }
  }

  None
}

#[derive(Clone, Debug, serde::Deserialize)]
pub struct GithubOidcTokenAud {
  pub permissions: Permissions,
}

pub async fn decode_json<T>(req: &mut Request<Body>) -> ApiResult<T>
where
  T: DeserializeOwned,
{
  let bytes = body::to_bytes(req.body_mut())
    .await
    .map_err(anyhow::Error::from)?;
  let data = serde_json::from_slice(&bytes).map_err(|error| {
    ApiError::MalformedRequest {
      msg: error.to_string().into(),
    }
  })?;
  Ok(data)
}

pub fn search(req: &Request<Body>) -> Option<&str> {
  req.query("query").map(|q| q.as_str())
}

pub fn pagination(req: &Request<Body>) -> (i64, i64) {
  let limit = req
    .query("limit")
    .and_then(|page| page.parse::<i64>().ok())
    .unwrap_or(100)
    .max(1)
    .min(100);
  let page = req
    .query("page")
    .and_then(|page| page.parse::<i64>().ok())
    .unwrap_or(1)
    .max(1);

  let start = (page * limit) - limit;

  (start, limit)
}

// Sanitize redirect urls
// - Remove origin from Url: https://evil.com -> /
// - Replace multiple slashes with one slash to remove prevent
//   relative url bypass: //evil.com/foo -> /foo
// - Remove /../ and /./ from path segments
pub fn sanitize_redirect_url(raw: &str) -> String {
  let base = Url::parse("http://localhost/").unwrap();
  let url = base.join(raw).unwrap_or(base.clone());

  let mut sanitized = "".to_string();

  if let Some(segments) = url.path_segments() {
    for seg in segments {
      if seg.is_empty() {
        continue;
      }

      sanitized.push_str(&format!("/{}", seg));
    }
  }

  if let Some(query) = url.query() {
    sanitized.push_str(&format!("?{}", query));
  }

  if sanitized.is_empty() {
    "/".to_string()
  } else {
    sanitized
  }
}

pub trait RequestIdExt {
  fn param_uuid(&self, name: &str) -> Result<Uuid, ApiError>;
  fn param_scope(&self) -> Result<ScopeName, ApiError>;
  fn param_package(&self) -> Result<PackageName, ApiError>;
  fn param_version(&self) -> Result<Version, ApiError>;
  fn param_version_or_latest(&self) -> Result<VersionOrLatest, ApiError>;
}

fn param<'a>(
  req: &'a Request<Body>,
  name: &str,
) -> Result<&'a String, ApiError> {
  req.param(name).ok_or_else(|| {
    let msg = format!("missing path parameter '{name}'").into();
    ApiError::MalformedRequest { msg }
  })
}

#[derive(Eq, PartialEq)]
pub enum VersionOrLatest {
  Version(Version),
  Latest,
}

impl std::fmt::Display for VersionOrLatest {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    match self {
      VersionOrLatest::Version(version) => std::fmt::Display::fmt(version, f),
      VersionOrLatest::Latest => f.write_str("latest"),
    }
  }
}

impl RequestIdExt for Request<Body> {
  fn param_uuid(&self, name: &str) -> Result<Uuid, ApiError> {
    let value = param(self, name)?;
    Uuid::parse_str(value).map_err(|err| {
      let msg =
        format!("failed to parse path parameter '{name}' as uuid: {err}")
          .into();
      ApiError::MalformedRequest { msg }
    })
  }

  fn param_scope(&self) -> Result<ScopeName, ApiError> {
    let value = param(self, "scope")?;
    ScopeName::try_from(value.as_str()).map_err(|err| {
      let msg = format!("failed to parse path parameter 'scope': {err}").into();
      ApiError::MalformedRequest { msg }
    })
  }

  fn param_package(&self) -> Result<PackageName, ApiError> {
    let value = param(self, "package")?;
    PackageName::try_from(value.as_str()).map_err(|err| {
      let msg =
        format!("failed to parse path parameter 'package': {err}").into();
      ApiError::MalformedRequest { msg }
    })
  }

  fn param_version(&self) -> Result<Version, ApiError> {
    let value = param(self, "version")?;
    Version::try_from(value.as_str()).map_err(|err| {
      let msg =
        format!("failed to parse path parameter 'version': {err}").into();
      ApiError::MalformedRequest { msg }
    })
  }

  fn param_version_or_latest(&self) -> Result<VersionOrLatest, ApiError> {
    let value = param(self, "version")?;
    if value == "latest" {
      Ok(VersionOrLatest::Latest)
    } else {
      let version = Version::try_from(value.as_str()).map_err(|err| {
        let msg =
          format!("failed to parse path parameter 'version': {err}").into();
        ApiError::MalformedRequest { msg }
      })?;
      Ok(VersionOrLatest::Version(version))
    }
  }
}

#[cfg(test)]
pub mod test {
  use crate::auth::GithubOauth2Client;
  use crate::buckets::BucketWithQueue;
  use crate::buckets::Buckets;
  use crate::db::EphemeralDatabase;
  use crate::db::NewGithubIdentity;
  use crate::db::{Database, NewUser, User};
  use crate::errors_internal::ApiErrorStruct;
  use crate::gcp::FakeGcsTester;
  use crate::util::sanitize_redirect_url;
  use crate::ApiError;
  use crate::MainRouterOptions;
  use hyper::http::HeaderName;
  use hyper::http::HeaderValue;
  use hyper::service::Service;
  use hyper::Body;
  use hyper::HeaderMap;
  use hyper::Response;
  use hyper::StatusCode;
  use routerify::RequestService;
  use routerify::RouteError;
  use serde::de::DeserializeOwned;
  use std::net::{Ipv4Addr, SocketAddr, SocketAddrV4};
  use url::Url;

  #[derive(Debug)]
  pub struct TestUser {
    pub user: User,
    pub token: String,
    pub github_name: String,
  }

  pub struct TestSetup {
    pub ephemeral_database: EphemeralDatabase,
    #[allow(dead_code)]
    pub gcs: FakeGcsTester,
    pub buckets: Buckets,
    pub user1: TestUser,
    pub user2: TestUser,
    pub user3: TestUser,
    pub staff_user: TestUser,
    #[allow(dead_code)]
    pub scope: crate::db::Scope,
    pub github_oauth2_client: GithubOauth2Client,
    pub service: RequestService<Body, ApiError>,
  }

  impl TestSetup {
    pub async fn new() -> Self {
      let ephemeral_database = EphemeralDatabase::create().await;
      let db = ephemeral_database.database.clone().unwrap();
      let gcs = FakeGcsTester::new().await;
      let publishing_bucket = gcs.create_bucket("publishing").await;
      let modules_bucket = gcs.create_bucket("modules").await;
      let docs_bucket = gcs.create_bucket("docs").await;
      let npm_bucket = gcs.create_bucket("npm").await;
      let buckets = Buckets {
        publishing_bucket: BucketWithQueue::new(publishing_bucket),
        modules_bucket: BucketWithQueue::new(modules_bucket),
        docs_bucket: BucketWithQueue::new(docs_bucket),
        npm_bucket: BucketWithQueue::new(npm_bucket),
      };
      let github_oauth2_client = GithubOauth2Client::new(
        oauth2::ClientId::new("".to_string()),
        Some(oauth2::ClientSecret::new("".to_string())),
        oauth2::AuthUrl::new(
          "https://github.com/login/oauth/authorize".to_string(),
        )
        .unwrap(),
        Some(
          oauth2::TokenUrl::new(
            "https://github.com/login/oauth/access_token".to_string(),
          )
          .unwrap(),
        ),
      );

      let user1 = Self::create_user(
        &db,
        NewUser {
          name: "User 1",
          email: None,
          avatar_url: "https://avatars0.githubusercontent.com/u/952?v=4",
          github_id: Some(101),
          is_blocked: false,
          is_staff: false,
        },
        "ry",
      )
      .await;

      let user2 = Self::create_user(
        &db,
        NewUser {
          name: "User 2",
          email: None,
          avatar_url: "",
          github_id: Some(102),
          is_blocked: false,
          is_staff: false,
        },
        "lucacasonato",
      )
      .await;

      let user3 = Self::create_user(
        &db,
        NewUser {
          name: "User 3",
          email: None,
          avatar_url: "",
          github_id: Some(103),
          is_blocked: false,
          is_staff: false,
        },
        "crowlkats",
      )
      .await;

      let staff_user = Self::create_user(
        &db,
        NewUser {
          name: "User 4",
          email: None,
          avatar_url: "",
          github_id: Some(104),
          is_blocked: false,
          is_staff: true,
        },
        "bartlomieju",
      )
      .await;

      let scope_name = "scope".try_into().unwrap();

      db.create_scope(&scope_name, user1.user.id).await.unwrap();
      let (scope, _, _) = db
        .update_scope_limits(&scope_name, Some(250), Some(200), Some(1000))
        .await
        .unwrap();

      db.add_bad_word_for_test("somebadword").await.unwrap();

      let router = crate::main_router(MainRouterOptions {
        database: db,
        buckets: buckets.clone(),
        github_client: github_oauth2_client.clone(),
        orama_client: None,
        email_sender: None,
        registry_url: "http://jsr-tests.test".parse().unwrap(),
        npm_url: "http://npm.jsr-tests.test".parse().unwrap(),
        publish_queue: None,           // no queue locally
        npm_tarball_build_queue: None, // no queue locally
        expose_api: true,              // api enabled
        expose_tasks: true,            // task endpoints enabled
      });

      let service = routerify::RequestServiceBuilder::new(router)
        .unwrap()
        .build(SocketAddr::V4(SocketAddrV4::new(Ipv4Addr::LOCALHOST, 8080)));

      Self {
        ephemeral_database,
        gcs,
        buckets,
        user1,
        user2,
        user3,
        staff_user,
        scope,
        github_oauth2_client,
        service,
      }
    }

    async fn create_user<'a>(
      db: &Database,
      new_user: NewUser<'a>,
      github_name: &str,
    ) -> TestUser {
      db.upsert_github_identity(NewGithubIdentity {
        github_id: new_user.github_id.unwrap(),
        access_token: None,
        access_token_expires_at: None,
        refresh_token: None,
        refresh_token_expires_at: None,
      })
      .await
      .unwrap();

      let user = db.insert_user(new_user).await.unwrap();

      let token = crate::token::create_token(
        db,
        user.id,
        crate::db::TokenType::Web,
        None,
        Some(chrono::Utc::now() + chrono::Duration::try_days(7).unwrap()),
        None,
      )
      .await
      .unwrap();

      TestUser {
        user,
        token,
        github_name: github_name.to_string(),
      }
    }

    pub fn db(&self) -> Database {
      self.ephemeral_database.database.clone().unwrap()
    }

    pub fn buckets(&self) -> Buckets {
      self.buckets.clone()
    }

    pub fn registry_url(&self) -> Url {
      Url::parse("http://jsr-tests.test").unwrap()
    }

    pub fn npm_url(&self) -> Url {
      Url::parse("http://npm.jsr-tests.test").unwrap()
    }

    pub fn http(&mut self) -> TestHttpClient {
      TestHttpClient {
        service: &mut self.service,
        auth: Some(&self.user1.token),
      }
    }

    pub fn unauthed_http(&mut self) -> TestHttpClient {
      TestHttpClient {
        service: &mut self.service,
        auth: None,
      }
    }
  }

  pub struct TestHttpClient<'s, 't> {
    service: &'s mut RequestService<Body, ApiError>,
    auth: Option<&'t str>,
  }

  impl<'s, 't> TestHttpClient<'s, 't> {
    pub fn get<U: AsRef<str>>(&'s mut self, uri: U) -> TestHttpCall<'s> {
      TestHttpCall::new(
        self.service,
        "GET",
        uri.as_ref().to_string(),
        self.auth,
      )
    }
    pub fn post<U: AsRef<str>>(&'s mut self, uri: U) -> TestHttpCall<'s> {
      TestHttpCall::new(
        self.service,
        "POST",
        uri.as_ref().to_string(),
        self.auth,
      )
    }
    pub fn delete<U: AsRef<str>>(&'s mut self, uri: U) -> TestHttpCall<'s> {
      TestHttpCall::new(
        self.service,
        "DELETE",
        uri.as_ref().to_string(),
        self.auth,
      )
    }
    #[allow(dead_code)]
    pub fn put<U: AsRef<str>>(&'s mut self, uri: U) -> TestHttpCall<'s> {
      TestHttpCall::new(
        self.service,
        "PUT",
        uri.as_ref().to_string(),
        self.auth,
      )
    }
    pub fn patch<U: AsRef<str>>(&'s mut self, uri: U) -> TestHttpCall<'s> {
      TestHttpCall::new(
        self.service,
        "PATCH",
        uri.as_ref().to_string(),
        self.auth,
      )
    }
  }

  pub struct TestHttpCall<'s> {
    service: &'s mut RequestService<Body, ApiError>,
    method: &'static str,
    uri: String,
    body: Body,
    headers: HeaderMap,
    token: Option<&'s str>,
    sudo: bool,
  }

  impl<'s> TestHttpCall<'s> {
    fn new(
      service: &'s mut RequestService<Body, ApiError>,
      method: &'static str,
      uri: String,
      token: Option<&'s str>,
    ) -> Self {
      Self {
        service,
        method,
        uri,
        body: Body::empty(),
        headers: HeaderMap::default(),
        token,
        sudo: false,
      }
    }

    /// overwrite the default user token for authentication
    pub fn token(mut self, token: Option<&'s str>) -> Self {
      self.token = token;
      self
    }

    /// overwrite the default user token for authentication
    pub fn sudo(mut self, sudo: bool) -> Self {
      self.sudo = sudo;
      self
    }

    pub fn body_json(mut self, body: serde_json::Value) -> Self {
      self.body = body.to_string().into();
      self
    }

    pub fn header(mut self, name: HeaderName, value: HeaderValue) -> Self {
      let prev = self.headers.insert(name.clone(), value);
      assert!(prev.is_none(), "{} already present in the header map", name);
      self
    }

    pub fn gzip(self) -> Self {
      self.header(hyper::header::CONTENT_ENCODING, "gzip".try_into().unwrap())
    }

    pub fn body(mut self, body: Body) -> Self {
      self.body = body;
      self
    }

    pub async fn call(self) -> Result<hyper::Response<Body>, RouteError> {
      let mut req = hyper::Request::builder().method(self.method).uri(self.uri);

      for (key, value) in self.headers.into_iter() {
        req = req.header(key.unwrap(), value)
      }
      if let Some(token) = self.token {
        req = req
          .header(hyper::header::AUTHORIZATION, &format!("Bearer {}", token));
      }

      if self.sudo {
        req = req
          .header(hyper::header::HeaderName::from_static("x-jsr-sudo"), "true");
      }

      let req = req.body(self.body).unwrap();
      self.service.call(req).await
    }
  }

  #[async_trait::async_trait]
  pub trait ApiResultExt {
    #[track_caller]
    async fn expect_json<T: DeserializeOwned>(
      &mut self,
      status: StatusCode,
    ) -> T;

    #[track_caller]
    async fn expect_ok<T: DeserializeOwned>(&mut self) -> T {
      self.expect_json(StatusCode::OK).await
    }

    #[track_caller]
    async fn expect_ok_no_content(&mut self);

    #[track_caller]
    async fn expect_err(&mut self, status: StatusCode) -> ApiErrorStruct {
      self.expect_json(status).await
    }

    #[track_caller]
    async fn expect_err_code(
      &mut self,
      status: StatusCode,
      code: &str,
    ) -> ApiErrorStruct {
      let err: ApiErrorStruct = self.expect_json(status).await;
      assert_eq!(err.code, code, "{}", err.message);
      err
    }
  }

  #[async_trait::async_trait]
  impl ApiResultExt for Response<Body> {
    #[track_caller]
    async fn expect_json<T: DeserializeOwned>(
      &mut self,
      status: StatusCode,
    ) -> T {
      let bytes = hyper::body::to_bytes(self.body_mut()).await.unwrap();
      let body = std::str::from_utf8(&bytes).expect("invalid utf8");
      assert_eq!(
        self.status(),
        status,
        "expected {} response, got {} with body: {}",
        status,
        self.status(),
        body
      );
      serde_json::from_str(body).unwrap()
    }

    #[track_caller]
    async fn expect_ok_no_content(&mut self) {
      let bytes = hyper::body::to_bytes(self.body_mut()).await.unwrap();
      let body = std::str::from_utf8(&bytes).expect("invalid utf8");
      assert_eq!(
        self.status(),
        StatusCode::NO_CONTENT,
        "expected {} response, got {} with body: {}",
        StatusCode::NO_CONTENT,
        self.status(),
        body
      );
    }
  }

  #[tokio::test]
  async fn harness_expectations() {
    let t = TestSetup::new().await;
    // This permissions set up is important - the scope member management tests
    // rely on it.
    assert!(!t.user1.user.is_staff);
    assert!(!t.user2.user.is_staff);
    assert!(!t.user3.user.is_staff);
    assert!(t.staff_user.user.is_staff);
  }

  #[test]
  fn sanitize_url_test() {
    assert_eq!(sanitize_redirect_url("/foo"), "/foo");
    assert_eq!(sanitize_redirect_url("//evil.com/bar"), "/bar");
    assert_eq!(
      sanitize_redirect_url("//evil.com//bar?foo=bar"),
      "/bar?foo=bar"
    );
    assert_eq!(sanitize_redirect_url("https://evil.com"), "/");
    assert_eq!(sanitize_redirect_url("/../foo"), "/foo");
    assert_eq!(sanitize_redirect_url("/../foo/../bar"), "/bar");
    assert_eq!(sanitize_redirect_url("/foo/./bar"), "/foo/bar");
  }
}
