// Copyright 2024 the JSR authors. All rights reserved. MIT license.
mod admin;
mod authorization;
mod errors;
mod package;
mod publishing_task;
mod scope;
mod self_user;
mod types;
mod users;
mod changes;


use hyper::Body;
use hyper::Response;
use package::global_list_handler;
use package::global_metrics_handler;
use package::global_stats_handler;
use routerify::Middleware;
use routerify::Router;

pub use self::errors::*;
pub use self::package::PublishQueue;
use self::publishing_task::publishing_task_router;
use self::self_user::self_user_router;
pub use self::types::*;

use self::admin::admin_router;
use self::authorization::authorization_router;
use self::scope::scope_router;
use self::users::users_router;
use self::changes::changes_router;

use crate::util;
use crate::util::CacheDuration;

pub fn api_router() -> Router<Body, ApiError> {
  Router::builder()
    .get(
      "/metrics",
      util::cache(
        CacheDuration::ONE_MINUTE,
        util::json(global_metrics_handler),
      ),
    )
    .middleware(Middleware::pre(util::auth_middleware))
    .scope("/admin", admin_router())
    .scope("/scopes", scope_router())
    .scope("/changes", changes_router())
    .scope("/user", self_user_router())
    .scope("/users", users_router())
    .scope("/authorizations", authorization_router())
    .scope("/publishing_tasks", publishing_task_router())
    .get("/packages", util::json(global_list_handler))
    .get(
      "/stats",
      util::cache(CacheDuration::ONE_MINUTE, util::json(global_stats_handler)),
    )
    .get(
      // todo: remove once CLI uses the new endpoint
      "/publish_status/:publishing_task_id",
      util::json(publishing_task::get_handler),
    )
    .get("/.well-known/openapi", openapi_handler)
    .build()
    .unwrap()
}

async fn openapi_handler(
  _: hyper::Request<Body>,
) -> util::ApiResult<Response<Body>> {
  let openapi = include_str!("../api.yml");
  let resp = Response::builder()
    .header("Content-Type", "application/x-yaml")
    .body(Body::from(openapi))
    .unwrap();
  Ok(resp)
}
