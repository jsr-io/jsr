// Copyright 2024 the JSR authors. All rights reserved. MIT license.
//! Tests of `shared::router` that go through the api's routes.

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
    let name = shared::ids::PackageName::new(name.to_owned()).unwrap();
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
