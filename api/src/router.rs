// Copyright 2024 the JSR authors. All rights reserved. MIT license.
use std::any::Any;
use std::any::TypeId;
use std::collections::HashMap;
use std::sync::Arc;

use axum::body::Body;
use axum::extract::RawPathParams;
use axum::middleware::Next;
use hyper::Method;
use hyper::Request;
use hyper::Response;
use hyper::StatusCode;
use hyper::Uri;
use hyper::header;
use tower::ServiceExt;
use tower::util::MapRequest;

pub type App = MapRequest<axum::Router, fn(Request<Body>) -> Request<Body>>;

#[derive(Default)]
pub struct Data(HashMap<TypeId, Box<dyn Any + Send + Sync>>);

impl Data {
  pub fn with<T: Send + Sync + 'static>(mut self, value: T) -> Self {
    self.0.insert(TypeId::of::<T>(), Box::new(value));
    self
  }
}

#[derive(Clone)]
struct SharedData(Arc<Data>);

#[derive(Clone)]
struct PathParams(HashMap<String, String>);

#[derive(Clone)]
struct Query(HashMap<String, String>);

pub trait RequestExt {
  fn data<T: Send + Sync + 'static>(&self) -> Option<&T>;
  fn param(&self, name: &str) -> Option<&String>;
  fn query(&self, name: &str) -> Option<&String>;
  fn context<T: Clone + Send + Sync + 'static>(&self) -> Option<T>;
}

impl RequestExt for Request<Body> {
  fn data<T: Send + Sync + 'static>(&self) -> Option<&T> {
    self
      .extensions()
      .get::<SharedData>()?
      .0
      .0
      .get(&TypeId::of::<T>())?
      .downcast_ref()
  }

  fn param(&self, name: &str) -> Option<&String> {
    self.extensions().get::<PathParams>()?.0.get(name)
  }

  fn query(&self, name: &str) -> Option<&String> {
    self.extensions().get::<Query>()?.0.get(name)
  }

  fn context<T: Clone + Send + Sync + 'static>(&self) -> Option<T> {
    self.extensions().get::<T>().cloned()
  }
}

/// Finishes a fully assembled router: shared data, path params and query
/// parameters on every request, a plain `404 Not Found` (and `204` for any
/// `OPTIONS`) when nothing matches, and paths matched with or without a
/// trailing slash.
pub fn app(router: axum::Router, data: Data) -> App {
  let router = if router.has_routes() {
    router.route_layer(axum::middleware::from_fn(request_meta))
  } else {
    router
  };
  let router = router
    .method_not_allowed_fallback(fallback)
    .fallback(fallback)
    .layer(axum::Extension(SharedData(Arc::new(data))));
  router.map_request(strip_trailing_slash as fn(_) -> _)
}

async fn request_meta(
  params: RawPathParams,
  mut req: Request<Body>,
  next: Next,
) -> Response<Body> {
  let params = params
    .iter()
    .map(|(name, value)| (name.to_owned(), value.to_owned()))
    .collect();
  let query = req
    .uri()
    .query()
    .map(|query| {
      url::form_urlencoded::parse(query.as_bytes())
        .into_owned()
        .collect()
    })
    .unwrap_or_default();
  req.extensions_mut().insert(PathParams(params));
  req.extensions_mut().insert(Query(query));
  next.run(req).await
}

async fn fallback(req: Request<Body>) -> Response<Body> {
  if req.method() == Method::OPTIONS {
    return Response::builder()
      .status(StatusCode::NO_CONTENT)
      .body(Body::empty())
      .unwrap();
  }
  Response::builder()
    .status(StatusCode::NOT_FOUND)
    .header(header::CONTENT_TYPE, "text/plain")
    .body(Body::from(
      StatusCode::NOT_FOUND.canonical_reason().unwrap(),
    ))
    .unwrap()
}

fn strip_trailing_slash(mut req: Request<Body>) -> Request<Body> {
  let Some(path) = req.uri().path().strip_suffix('/') else {
    return req;
  };
  if path.is_empty() {
    return req;
  }
  let path_and_query = match req.uri().query() {
    Some(query) => format!("{path}?{query}"),
    None => path.to_owned(),
  };
  let mut parts = req.uri().clone().into_parts();
  parts.path_and_query = path_and_query.parse().ok();
  if let Ok(uri) = Uri::from_parts(parts) {
    *req.uri_mut() = uri;
  }
  req
}

#[cfg(test)]
mod tests {
  use crate::util::test::ApiResultExt;
  use crate::util::test::TestSetup;
  use hyper::StatusCode;
  use hyper::header;

  async fn text(resp: hyper::Response<axum::body::Body>) -> String {
    let bytes = axum::body::to_bytes(resp.into_body(), usize::MAX)
      .await
      .unwrap();
    String::from_utf8(bytes.to_vec()).unwrap()
  }

  #[tokio::test]
  async fn trailing_slash() {
    let mut t = TestSetup::new().await;
    let mut resp = t.http().get("/api/scopes/scope/").call().await.unwrap();
    let scope: serde_json::Value = resp.expect_ok().await;
    assert_eq!(scope["scope"], "scope");
  }

  #[tokio::test]
  async fn not_found() {
    let mut t = TestSetup::new().await;
    let resp = t.http().get("/api/does_not_exist").call().await.unwrap();
    assert_eq!(resp.status(), StatusCode::NOT_FOUND);
    assert_eq!(resp.headers()[header::CONTENT_TYPE], "text/plain");
    assert_eq!(text(resp).await, "Not Found");

    let resp = t.http().put("/api/scopes/scope").call().await.unwrap();
    assert_eq!(resp.status(), StatusCode::NOT_FOUND);
  }

  #[tokio::test]
  async fn options() {
    let mut t = TestSetup::new().await;
    let resp = t.http().options("/api/scopes/scope").call().await.unwrap();
    assert_eq!(resp.status(), StatusCode::NO_CONTENT);
    let resp = t.http().options("/does_not_exist").call().await.unwrap();
    assert_eq!(resp.status(), StatusCode::NO_CONTENT);
  }

  #[tokio::test]
  async fn auth_covers_every_api_route() {
    let mut t = TestSetup::new().await;
    let mut resp = t
      .http()
      .get("/api/metrics")
      .token(Some("invalid"))
      .call()
      .await
      .unwrap();
    resp
      .expect_err_code(StatusCode::UNAUTHORIZED, "invalidBearerToken")
      .await;
  }

  #[tokio::test]
  async fn query_and_params() {
    let mut t = TestSetup::new().await;
    for name in ["bar", "foo"] {
      let name = crate::ids::PackageName::new(name.to_owned()).unwrap();
      t.db().create_package(&t.scope.scope, &name).await.unwrap();
    }

    let mut names = Vec::new();
    for page in [1, 2] {
      let mut resp = t
        .http()
        .get(format!("/api/scopes/scope/packages?limit=1&page={page}"))
        .call()
        .await
        .unwrap();
      let list: serde_json::Value = resp.expect_ok().await;
      assert_eq!(list["total"], 2);
      let items = list["items"].as_array().unwrap();
      assert_eq!(items.len(), 1);
      names.push(items[0]["name"].as_str().unwrap().to_owned());
    }
    names.sort();
    assert_eq!(names, ["bar", "foo"]);
  }
}
