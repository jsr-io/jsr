// Copyright 2024 the JSR authors. All rights reserved. MIT license.
use crate::analysis::RegistryLoader;
use crate::buckets::Buckets;
use crate::emails::EmailArgs;
use crate::emails::EmailSender;
use crate::NpmUrl;
use crate::RegistryUrl;
use hyper::Body;
use hyper::Request;
use routerify::prelude::RequestExt;
use routerify::Router;
use std::borrow::Cow;
use std::sync::Arc;
use tracing::field;
use tracing::instrument;
use tracing::Instrument;
use tracing::Span;

use crate::db::*;
use crate::iam::ReqIamExt;
use crate::publish::publish_task;
use crate::util;
use crate::util::decode_json;
use crate::util::pagination;
use crate::util::search;
use crate::util::ApiResult;
use crate::util::RequestIdExt;

use super::types::*;
use super::ApiError;
use super::PublishQueue;

pub fn admin_router() -> Router<Body, ApiError> {
  Router::builder()
    .get("/aliases", util::auth(util::json(list_aliases)))
    .post("/aliases", util::auth(util::json(create_alias)))
    .get("/users", util::auth(util::json(list_users)))
    .get("/users/waitlisted", util::auth(util::json(list_waitlisted)))
    .patch("/users/:user_id", util::auth(util::json(update_user)))
    .post(
      "/users/:user_id/waitlist_accept",
      util::auth(util::json(waitlist_accept_user)),
    )
    .get("/scopes", util::auth(util::json(list_scopes)))
    .patch("/scopes/:scope", util::auth(util::json(patch_scopes)))
    .get(
      "/publishing_tasks",
      util::auth(util::json(list_publishing_tasks)),
    )
    .post(
      "/publishing_tasks/:publishing_task/requeue",
      util::auth(util::json(requeue_publishing_tasks)),
    )
    .build()
    .unwrap()
}

#[instrument(name = "GET /api/admin/aliases", skip(req), err)]
pub async fn list_aliases(req: Request<Body>) -> ApiResult<Vec<ApiAlias>> {
  let iam = req.iam();
  iam.check_admin_access()?;

  let db = req.data::<Database>().unwrap();
  let (start, limit) = pagination(&req);
  let maybe_search = search(&req);
  let alias = db.list_aliases(start, limit, maybe_search).await?;

  Ok(alias.into_iter().map(|alias| alias.into()).collect())
}

#[instrument(
  name = "POST /api/admin/aliases",
  skip(req),
  err,
  fields(name, major_version)
)]
pub async fn create_alias(mut req: Request<Body>) -> ApiResult<ApiAlias> {
  let iam = req.iam();
  iam.check_admin_access()?;

  let ApiCreateAliasRequest {
    name,
    major_version,
    target,
  } = decode_json(&mut req).await?;

  let span = Span::current();
  span.record("name", &name);
  span.record("major_version", major_version);

  let db = req.data::<Database>().unwrap();
  let alias = db.create_alias(&name, major_version, target).await?;

  Ok(alias.into())
}

#[instrument(name = "GET /api/admin/users", skip(req), err)]
pub async fn list_users(req: Request<Body>) -> ApiResult<ApiList<ApiFullUser>> {
  let iam = req.iam();
  iam.check_admin_access()?;

  let db = req.data::<Database>().unwrap();
  let (start, limit) = pagination(&req);
  let maybe_search = search(&req);

  let (total, users) = db.list_users(start, limit, maybe_search).await?;
  Ok(ApiList {
    items: users.into_iter().map(|user| user.into()).collect(),
    total,
  })
}

#[instrument(name = "GET /api/admin/users/waitlisted", skip(req), err)]
pub async fn list_waitlisted(
  req: Request<Body>,
) -> ApiResult<ApiList<ApiFullUser>> {
  let iam = req.iam();
  iam.check_admin_access()?;

  let db = req.data::<Database>().unwrap();
  let (start, limit) = pagination(&req);
  let maybe_search = search(&req);

  let (total_users, users) =
    db.list_users_waitlisted(start, limit, maybe_search).await?;

  Ok(ApiList {
    items: users.into_iter().map(|user| user.into()).collect(),
    total: total_users,
  })
}

#[instrument(
  name = "PATCH /api/admin/users/:user_id",
  skip(req),
  err,
  fields(user_id)
)]
pub async fn update_user(mut req: Request<Body>) -> ApiResult<ApiFullUser> {
  let iam = req.iam();
  iam.check_admin_access()?;

  let user_id = req.param_uuid("user_id")?;
  Span::current().record("user_id", &field::display(&user_id));
  let ApiAdminUpdateUserRequest {
    is_staff,
    is_blocked,
    scope_limit,
  } = decode_json(&mut req).await?;
  let db = req.data::<Database>().unwrap();

  let mut updated_user = None;

  if let Some(is_staff) = is_staff {
    updated_user = Some(db.user_set_staff(user_id, is_staff).await?);
  }
  if let Some(is_blocked) = is_blocked {
    updated_user = Some(db.user_set_blocked(user_id, is_blocked).await?);
  }
  if let Some(scope_limit) = scope_limit {
    updated_user = Some(db.user_set_scope_limit(user_id, scope_limit).await?);
  }

  if let Some(updated_user) = updated_user {
    Ok(updated_user.into())
  } else {
    Err(ApiError::MalformedRequest {
      msg: "missing 'is_staff', 'is_blocked' or 'scope_limit' parameter".into(),
    })
  }
}

#[instrument(
  name = "PATCH /api/admin/users/:user_id/waitlist_accept",
  skip(req),
  err,
  fields(user_id)
)]
pub async fn waitlist_accept_user(
  req: Request<Body>,
) -> ApiResult<ApiFullUser> {
  let iam = req.iam();
  iam.check_admin_access()?;

  let user_id = req.param_uuid("user_id")?;
  Span::current().record("user_id", &field::display(&user_id));
  let db = req.data::<Database>().unwrap();
  let email_sender = req.data::<Option<EmailSender>>().unwrap();
  let registry_url = req.data::<RegistryUrl>().unwrap();

  let user = db.get_user(user_id).await?.ok_or(ApiError::UserNotFound)?;
  if user.waitlist_accepted_at.is_some() {
    return Ok(user.into());
  }

  let updated_user = db.user_waitlist_accept(user_id).await?;

  if let Some(email_sender) = email_sender {
    if let Some(email_addr) = user.email {
      let email = EmailArgs::WaitlistAccept {
        name: Cow::Borrowed(&updated_user.name),
        registry_url: Cow::Borrowed(registry_url.0.as_str()),
        registry_name: Cow::Borrowed(&email_sender.from_name),
        support_email: Cow::Borrowed(&email_sender.from),
      };
      email_sender.send(email_addr, email).await?;
    }
  }

  Ok(updated_user.into())
}

#[instrument(name = "GET /api/admin/scopes", skip(req), err)]
pub async fn list_scopes(
  req: Request<Body>,
) -> ApiResult<ApiList<ApiFullScope>> {
  let iam = req.iam();
  iam.check_admin_access()?;

  let db = req.data::<Database>().unwrap();
  let (start, limit) = pagination(&req);
  let maybe_search = search(&req);

  let (total, scopes) = db.list_scopes(start, limit, maybe_search).await?;
  Ok(ApiList {
    items: scopes.into_iter().map(|scope| scope.into()).collect(),
    total,
  })
}

#[instrument(
  name = "PATCH /api/admin/scopes/:scope",
  skip(req),
  err,
  fields(scope)
)]
pub async fn patch_scopes(mut req: Request<Body>) -> ApiResult<ApiFullScope> {
  let iam = req.iam();
  iam.check_admin_access()?;

  let scope = req.param_scope()?;
  Span::current().record("scope", &field::display(&scope));

  let ApiAdminUpdateScopeRequest {
    package_limit,
    new_package_per_week_limit,
    publish_attempts_per_week_limit,
  } = decode_json(&mut req).await?;

  let db = req.data::<Database>().unwrap();

  if package_limit.is_none()
    && new_package_per_week_limit.is_none()
    && publish_attempts_per_week_limit.is_none()
  {
    return Err(ApiError::MalformedRequest {
      msg: "missing 'packageLimit', 'newPackagePerWeekLimit' or 'publishAttemptsPerWeekLimit' parameter".into(),
    });
  }

  let scope = db
    .update_scope_limits(
      &scope,
      package_limit,
      new_package_per_week_limit,
      publish_attempts_per_week_limit,
    )
    .await?;

  Ok(scope.into())
}

#[instrument(name = "GET /api/admin/publishing_tasks", skip(req), err)]
pub async fn list_publishing_tasks(
  req: Request<Body>,
) -> ApiResult<ApiList<ApiPublishingTask>> {
  let iam = req.iam();
  iam.check_admin_access()?;

  let db = req.data::<Database>().unwrap();
  let (start, limit) = pagination(&req);
  let maybe_search = search(&req);

  let (total, publishing_tasks) =
    db.list_publishing_tasks(start, limit, maybe_search).await?;

  Ok(ApiList {
    items: publishing_tasks
      .into_iter()
      .map(|task| task.into())
      .collect(),
    total,
  })
}

#[instrument(
  name = "POST /api/admin/publishing_tasks/:publishing_task/requeue",
  skip(req),
  err
  fields(publishing_task)
)]
pub async fn requeue_publishing_tasks(req: Request<Body>) -> ApiResult<()> {
  let iam = req.iam();
  iam.check_admin_access()?;

  let publishing_task_id = req.param_uuid("publishing_task")?;
  Span::current()
    .record("publishing_task", &field::display(&publishing_task_id));

  let db = req.data::<Database>().unwrap().clone();
  let task = db
    .get_publishing_task(publishing_task_id)
    .await?
    .ok_or(ApiError::PublishNotFound)?;

  if task.status == PublishingTaskStatus::Processing {
    db.update_publishing_task_status(
      publishing_task_id,
      PublishingTaskStatus::Processing,
      PublishingTaskStatus::Pending,
      None,
    )
    .await?;
  }

  let publish_queue = req.data::<PublishQueue>().unwrap().0.clone();

  if let Some(queue) = publish_queue {
    let body = serde_json::to_vec(&publishing_task_id).unwrap();
    queue.task_buffer(None, Some(body.into())).await?;
  } else {
    let buckets = req.data::<Buckets>().unwrap().clone();
    let registry = req.data::<Arc<dyn RegistryLoader>>().unwrap().clone();
    let npm_url = req.data::<NpmUrl>().unwrap().0.clone();

    let span = Span::current();
    let fut = publish_task(publishing_task_id, buckets, registry, npm_url, db)
      .instrument(span);
    tokio::spawn(fut);
  }

  Ok(())
}

#[cfg(test)]
mod tests {
  use crate::api::ApiFullScope;
  use crate::api::ApiFullUser;
  use crate::api::ApiList;
  use crate::util::test::ApiResultExt;
  use crate::util::test::TestSetup;
  use serde_json::json;

  #[tokio::test]
  async fn list_users() {
    let mut t = TestSetup::new().await;

    let token = t.staff_user.token.clone();
    let users = t
      .http()
      .get("/api/admin/users")
      .token(Some(&token))
      .call()
      .await
      .unwrap()
      .expect_ok::<ApiList<ApiFullUser>>()
      .await;
    assert_eq!(users.items.len(), 5);

    let path = format!("/api/admin/users?query={}", t.user2.user.id);
    let users = t
      .http()
      .get(path)
      .token(Some(&token))
      .call()
      .await
      .unwrap()
      .expect_ok::<ApiList<ApiFullUser>>()
      .await;
    assert_eq!(users.items.len(), 1);
    assert_eq!(users.items[0].id, t.user2.user.id);
  }

  #[tokio::test]
  async fn scope_management() {
    let mut t = TestSetup::new().await;

    assert_eq!(t.scope.package_limit, 250);
    assert_eq!(t.scope.new_package_per_week_limit, 200);
    assert_eq!(t.scope.publish_attempts_per_week_limit, 1000);

    let path = format!("/api/admin/scopes/{}", t.scope.scope);
    let token = t.staff_user.token.clone();
    let res_scope = t
      .http()
      .patch(path)
      .body_json(json!({
        "packageLimit": 101,
        "newPackagePerWeekLimit": 101,
        "publishAttemptsPerWeekLimit": 101,
      }))
      .token(Some(&token))
      .call()
      .await
      .unwrap()
      .expect_ok::<ApiFullScope>()
      .await;
    assert_eq!(res_scope.quotas.package_limit, 101);
    assert_eq!(res_scope.quotas.new_package_per_week_limit, 101);
    assert_eq!(res_scope.quotas.publish_attempts_per_week_limit, 101);
  }
}
