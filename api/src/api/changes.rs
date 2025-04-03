// Copyright 2024 the JSR authors. All rights reserved. MIT license.
use hyper::{Body, Request};
use routerify::prelude::*;
use serde::{Deserialize, Serialize};
use tracing::instrument;

use crate::{
  db::{Change, Database},
  util::{pagination, ApiResult},
};

#[derive(Serialize, Deserialize)]
pub struct ApiChange {
  pub seq: i64,
  pub r#type: String,
  pub id: String,
  pub changes: serde_json::Value,
}

impl From<Change> for ApiChange {
  fn from(change: Change) -> Self {
    Self {
      seq: change.seq,
      r#type: change.change_type.to_string(),
      id: format!("@jsr/{}__{}", change.scope_name, change.package_name),
      changes: serde_json::from_str(&change.data).unwrap(),
    }
  }
}

#[instrument(name = "GET /api/_changes", skip(req), err)]
pub async fn list_changes(req: Request<Body>) -> ApiResult<Vec<ApiChange>> {
  let db = req.data::<Database>().unwrap();
  let (start, limit) = pagination(&req);
  let changes = db.list_changes(start, limit).await?;
  Ok(changes.into_iter().map(ApiChange::from).collect())
}

#[cfg(test)]
mod tests {
  use super::ApiChange;
  use crate::db::ChangeType;
  use crate::ids::PackageName;
  use crate::ids::ScopeName;
  use crate::util::test::ApiResultExt;
  use crate::util::test::TestSetup;
  use serde_json::json;

  #[tokio::test]
  async fn list_empty_changes() {
    let mut t = TestSetup::new().await;

    let changes = t
      .http()
      .get("/api/_changes")
      .call()
      .await
      .unwrap()
      .expect_ok::<Vec<ApiChange>>()
      .await;

    assert!(changes.is_empty());
  }

  #[tokio::test]
  async fn list_single_change() {
    let mut t = TestSetup::new().await;

    t.ephemeral_database
      .create_change(
        ChangeType::PackageVersionAdded,
        &ScopeName::new("test-scope".to_string()).unwrap(),
        &PackageName::new("test-package".to_string()).unwrap(),
        json!({
          "version": "1.0.0"
        }),
      )
      .await
      .unwrap();

    let changes = t
      .http()
      .get("/api/_changes")
      .call()
      .await
      .unwrap()
      .expect_ok::<Vec<ApiChange>>()
      .await;

    assert_eq!(changes.len(), 1);
    let change = &changes[0];
    assert_eq!(change.r#type, ChangeType::PackageVersionAdded.to_string());
    assert_eq!(change.id, "@jsr/test-scope__test-package");
    assert_eq!(change.changes["version"], "1.0.0");
  }

  #[tokio::test]
  async fn list_changes_pagination() {
    let mut t = TestSetup::new().await;

    // Create two changes
    t.ephemeral_database
      .create_change(
        ChangeType::PackageVersionAdded,
        &ScopeName::new("test-scope".to_string()).unwrap(),
        &PackageName::new("test-package-1".to_string()).unwrap(),
        json!({
          "name": "test-package-1",
        }),
      )
      .await
      .unwrap();

    t.ephemeral_database
      .create_change(
        ChangeType::PackageVersionAdded,
        &ScopeName::new("test-scope".to_string()).unwrap(),
        &PackageName::new("test-package-2".to_string()).unwrap(),
        json!({
          "version": "1.0.0",
        }),
      )
      .await
      .unwrap();

    // Test limit parameter
    let changes = t
      .http()
      .get("/api/_changes?limit=1&since=0")
      .call()
      .await
      .unwrap()
      .expect_ok::<Vec<ApiChange>>()
      .await;

    assert_eq!(changes.len(), 1);
    assert_eq!(changes[0].id, "@jsr/test-scope__test-package-1");

    // Test since parameter
    let changes = t
      .http()
      .get(format!("/api/_changes?since={}", changes[0].seq))
      .call()
      .await
      .unwrap()
      .expect_ok::<Vec<ApiChange>>()
      .await;

    assert_eq!(changes.len(), 1);
    assert_eq!(changes[0].id, "@jsr/test-scope__test-package-2");

    // Test since + limit combination
    let changes = t
      .http()
      .get("/api/_changes?since=0&limit=1")
      .call()
      .await
      .unwrap()
      .expect_ok::<Vec<ApiChange>>()
      .await;

    assert_eq!(changes.len(), 1);
    assert_eq!(changes[0].id, "@jsr/test-scope__test-package-1");
  }
}
