// Copyright 2024 the JSR authors. All rights reserved. MIT license.
mod admin;
mod authorization;
mod errors;
mod hooks;
pub mod package;
mod publishing_task;
mod scope;
mod self_user;
mod tickets;
mod types;
mod users;

pub use self::errors::*;
pub use self::hooks::InboundTrustedAuthservId;
pub use self::hooks::PostmarkWebhookPassword;
pub use self::package::PublishQueue;
pub use self::package::RegistryMetadataCache;
use self::publishing_task::publishing_task_router;
use self::self_user::self_user_router;
pub use self::types::*;
use crate::api::hooks::hooks_router;
use crate::api::tickets::tickets_router;
use axum::Router;
use axum::body::Body;
use axum::middleware;
use axum::routing::get;
use hyper::Response;
use package::global_list_handler;
use package::global_metrics_handler;
use package::global_stats_handler;

use self::admin::admin_router;
use self::authorization::authorization_router;
use self::scope::scope_router;
use self::users::users_router;

use crate::util;
use crate::util::CacheDuration;

pub fn api_router() -> Router {
  let router = Router::new()
    .route(
      "/metrics",
      get(util::cache(
        CacheDuration::ONE_MINUTE,
        util::json(global_metrics_handler),
      )),
    )
    .nest("/admin", admin_router())
    .nest("/scopes", scope_router())
    .nest("/user", self_user_router())
    .nest("/users", users_router())
    .nest("/authorizations", authorization_router())
    .nest("/publishing_tasks", publishing_task_router())
    .route(
      "/packages",
      get(util::cache(
        CacheDuration::FIVE_MINUTES,
        util::json(global_list_handler),
      )),
    )
    .route(
      "/stats",
      get(util::cache(
        CacheDuration::ONE_HOUR,
        util::json(global_stats_handler),
      )),
    )
    .route(
      // todo: remove once CLI uses the new endpoint
      // Never cache: `deno publish` polls this for live status, and a cached
      // non-terminal status would make it hang until the entry expired.
      "/publish_status/{publishing_task_id}",
      get(util::no_store(util::json(publishing_task::get_handler))),
    )
    .nest("/tickets", tickets_router())
    .nest("/hooks", hooks_router())
    .route("/.well-known/openapi", get(openapi_handler));

  // jemalloc-backed debug endpoints are only available in the native build.
  #[cfg(not(target_arch = "wasm32"))]
  let router = router
    .route(
      "/debug/mem_stats",
      get(util::auth(crate::jemalloc_profiling::mem_stats_handler)),
    )
    .route(
      "/debug/mem_dump",
      get(util::auth(crate::jemalloc_profiling::heap_profile_handler)),
    );

  router.route_layer(middleware::from_fn(util::auth_layer))
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
