use crate::api::ApiError;
use hyper::{Body, Request};
use routerify::Router;
use routerify::prelude::*;
use serde::Serialize;

use crate::{
    db::{Change, Database},
    util::{self, pagination, ApiResult},
};


#[derive(Serialize)]
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
            id: change.package_id,
            changes: serde_json::from_str(&change.data).unwrap(),
        }
    }
}

pub fn changes_router() -> Router<Body, ApiError> {
    Router::builder()
        .get("/_changes", util::json(list_changes))
        .build()
        .unwrap()
}

async fn list_changes(req: Request<Body>) -> ApiResult<Vec<ApiChange>> {
    let db = req.data::<Database>().unwrap();
    let (start, limit) = pagination(&req);
    let changes = db.list_changes(start, limit).await?;
    Ok(changes.into_iter().map(ApiChange::from).collect())
}
