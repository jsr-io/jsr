// Copyright 2024 the JSR authors. All rights reserved. MIT license.
use crate::FallbackRegistryUrl;
use crate::NpmUrl;
use crate::RegistryUrl;
use crate::external::algolia::AlgoliaClient;
use crate::s3::Buckets;
use hyper::Body;
use hyper::Request;
use routerify::Router;
use routerify::prelude::RequestExt;
use routerify_query::RequestQueryExt;
use tracing::Instrument;
use tracing::Span;
use tracing::field;
use tracing::instrument;

use std::borrow::Cow;
use std::collections::HashMap;

use crate::analysis::PackageAnalysisData;
use crate::analysis::analyze_package;
use crate::db::*;
use crate::emails;
use crate::emails::EmailArgs;
use crate::emails::EmailQueue;
use crate::emails::EmailSender;
use crate::emails::EmailThread;
use crate::iam::ReqIamExt;
use crate::ids::PackagePath;
use crate::ids::ScopeDescription;
use crate::publish::publish_task;
use crate::util;
use crate::util::ApiResult;
use crate::util::LicenseStore;
use crate::util::RequestIdExt;
use crate::util::decode_json;
use crate::util::pagination;
use crate::util::search;
use crate::util::sort;

use super::ApiError;
use super::PublishQueue;
use super::map_unique_violation;
use super::types::*;

pub fn admin_router() -> Router<Body, ApiError> {
  Router::builder()
    .get("/users", util::auth(util::json(list_users)))
    .patch("/users/:user_id", util::auth(util::json(update_user)))
    .post(
      "/users/:user_id/tickets",
      util::auth(util::json(create_outreach_ticket)),
    )
    .get("/scopes", util::auth(util::json(list_scopes)))
    .post("/scopes", util::auth(util::json(assign_scope)))
    .patch("/scopes/:scope", util::auth(util::json(patch_scopes)))
    .get("/packages", util::auth(util::json(list_packages)))
    .post(
      "/packages/:scope/:package/:version/recompute_meta",
      util::auth(util::json(recompute_package_version_meta)),
    )
    .get(
      "/publishing_tasks",
      util::auth(util::json(list_publishing_tasks)),
    )
    .post(
      "/publishing_tasks/:publishing_task/requeue",
      util::auth(util::json(requeue_publishing_tasks)),
    )
    .get("/tickets", util::auth(util::json(list_tickets)))
    .patch("/tickets/:id", util::auth(util::json(patch_ticket)))
    .get("/audit_logs", util::auth(util::json(list_audit_logs)))
    .build()
    .unwrap()
}

#[instrument(name = "GET /api/admin/users", skip(req))]
pub async fn list_users(req: Request<Body>) -> ApiResult<ApiList<ApiFullUser>> {
  let iam = req.iam();
  iam.check_admin_access()?;

  let db = req.data::<Database>().unwrap();
  let (start, limit) = pagination(&req);
  let maybe_search = search(&req);
  let maybe_sort = sort(&req);

  let (total, users) = db
    .list_users(start, limit, maybe_search, maybe_sort)
    .await?;
  Ok(ApiList {
    items: users.into_iter().map(|user| user.into()).collect(),
    total,
  })
}

#[instrument(
  name = "PATCH /api/admin/users/:user_id",
  skip(req),
  fields(user_id)
)]
pub async fn update_user(mut req: Request<Body>) -> ApiResult<ApiFullUser> {
  let user_id = req.param_uuid("user_id")?;
  Span::current().record("user_id", field::display(&user_id));
  let ApiAdminUpdateUserRequest {
    is_staff,
    is_blocked,
    scope_limit,
  } = decode_json(&mut req).await?;
  let db = req.data::<Database>().unwrap();

  let iam = req.iam();
  let staff = iam.check_admin_access()?;

  let mut updated_user = None;

  if let Some(is_staff) = is_staff {
    updated_user = Some(db.user_set_staff(&staff.id, user_id, is_staff).await?);
  }
  if let Some(is_blocked) = is_blocked {
    updated_user =
      Some(db.user_set_blocked(&staff.id, user_id, is_blocked).await?);
  }
  if let Some(scope_limit) = scope_limit {
    updated_user = Some(
      db.user_set_scope_limit(&staff.id, user_id, scope_limit)
        .await?,
    );
  }

  if let Some(updated_user) = updated_user {
    Ok(updated_user.into())
  } else {
    Err(ApiError::MalformedRequest {
      msg: "missing 'is_staff', 'is_blocked' or 'scope_limit' parameter".into(),
    })
  }
}

/// Opens a ticket addressed to a user, so that staff can start a conversation
/// rather than only answer one. The user is notified by email when they have
/// one; either way the ticket shows up in their account and unread badge, and
/// replying works exactly as on a ticket they opened themselves.
#[instrument(name = "POST /api/admin/users/:user_id/tickets", skip(req))]
pub async fn create_outreach_ticket(
  mut req: Request<Body>,
) -> ApiResult<ApiTicket> {
  let user_id = req.param_uuid("user_id")?;
  Span::current().record("user_id", field::display(&user_id));
  let ApiAdminNewOutreachTicketRequest {
    subject,
    message,
    meta,
  } = decode_json(&mut req).await?;
  let db = req.data::<Database>().unwrap();

  let iam = req.iam();
  let staff = iam.check_admin_access()?;

  let subject = subject.trim();
  if subject.is_empty() {
    return Err(ApiError::MalformedRequest {
      msg: "'subject' must not be empty".into(),
    });
  }
  if message.trim().is_empty() {
    return Err(ApiError::TicketMessageEmpty);
  }
  let meta = meta.unwrap_or_else(|| serde_json::json!({}));
  if !meta.is_object() {
    return Err(ApiError::TicketMetaNotValid);
  }

  let user = db.get_user(user_id).await?.ok_or(ApiError::UserNotFound)?;

  let email_sender = req.data::<Option<EmailSender>>().unwrap();
  let registry_url = req.data::<RegistryUrl>().unwrap();

  // Generated before the insert so the stored row and the header on the email
  // announcing it agree, which is what makes the user's reply threadable. Only
  // recorded when an email actually goes out: a user with no address on file
  // gets the ticket on the web alone.
  let email_message_id = match (&email_sender, &user.email) {
    (Some(_), Some(_)) => {
      Some(super::tickets::new_email_message_id(registry_url))
    }
    _ => None,
  };

  let (ticket, user, message) = db
    .create_staff_outreach_ticket(
      &staff.id,
      user_id,
      subject,
      meta,
      &message,
      email_message_id.as_deref(),
    )
    .await?;

  if let Some(email) = &user.email
    && let Some(email_sender) = email_sender
    && let Some(email_message_id) = &email_message_id
  {
    let email_args = EmailArgs::SupportTicketOutreach {
      name: Cow::Borrowed(&user.name),
      ticket_id: Cow::Owned(ticket.id.to_string()),
      ticket_number: Cow::Borrowed(&ticket.ticket_number),
      subject: Cow::Borrowed(subject),
      content: Cow::Borrowed(&message.0.message),
      registry_url: Cow::Borrowed(registry_url.0.as_str()),
      registry_name: Cow::Borrowed(&email_sender.from_name),
      support_email: Cow::Borrowed(&email_sender.from),
    };
    // Logged rather than returned: the ticket is already open, and failing the
    // request would have staff open a second one. The sweeper re-drives
    // anything left unsent.
    if let Err(err) = emails::enqueue(
      db,
      email_sender,
      req.data::<EmailQueue>().unwrap(),
      email.clone(),
      email_args,
      Some(EmailThread {
        message_id: email_message_id,
        in_reply_to: None,
        references: vec![],
      }),
    )
    .await
    {
      tracing::error!("failed to queue email: {:?}", err);
    }
  }

  Ok(ApiTicket::for_viewer(
    (ticket, Some(user), vec![message]),
    true,
  ))
}

#[instrument(name = "GET /api/admin/scopes", skip(req))]
pub async fn list_scopes(
  req: Request<Body>,
) -> ApiResult<ApiList<ApiFullScope>> {
  let iam = req.iam();
  iam.check_admin_access()?;

  let db = req.data::<Database>().unwrap();
  let (start, limit) = pagination(&req);
  let maybe_search = search(&req);
  let maybe_sort = sort(&req);

  let (total, scopes) = db
    .list_scopes(start, limit, maybe_search, maybe_sort)
    .await?;
  Ok(ApiList {
    items: scopes.into_iter().map(|scope| scope.into()).collect(),
    total,
  })
}

#[instrument(name = "PATCH /api/admin/scopes/:scope", skip(req), fields(scope))]
pub async fn patch_scopes(mut req: Request<Body>) -> ApiResult<ApiFullScope> {
  let scope = req.param_scope()?;
  Span::current().record("scope", field::display(&scope));

  let ApiAdminUpdateScopeRequest {
    package_limit,
    new_package_per_week_limit,
    publish_attempts_per_week_limit,
  } = decode_json(&mut req).await?;

  let iam = req.iam();
  let staff = iam.check_admin_access()?;

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
      &staff.id,
      &scope,
      package_limit,
      new_package_per_week_limit,
      publish_attempts_per_week_limit,
    )
    .await?;

  Ok(scope.into())
}

#[instrument(
  name = "POST /api/admin/scopes",
  skip(req),
  fields(scope, user_id)
)]
pub async fn assign_scope(mut req: Request<Body>) -> ApiResult<ApiScope> {
  let ApiAssignScopeRequest { scope, user_id } = decode_json(&mut req).await?;
  Span::current().record("scope", field::display(&scope));
  Span::current().record("user_id", field::display(&user_id));

  let iam = req.iam();
  let staff = iam.check_admin_access()?;

  let db = req.data::<Database>().unwrap();

  let scope_without_hyphens = scope.replace('-', "");

  if db.check_is_bad_word(&scope_without_hyphens).await? {
    return Err(ApiError::ScopeNameNotAllowed);
  }

  let scope = db
    .create_scope(
      &staff.id,
      true,
      &scope,
      user_id,
      &ScopeDescription::default(),
    )
    .await
    .map_err(|e| map_unique_violation(e, ApiError::ScopeAlreadyExists))?;

  Ok(scope.into())
}

#[instrument(name = "GET /api/admin/packages", skip(req))]
pub async fn list_packages(
  req: Request<Body>,
) -> ApiResult<ApiList<ApiPackage>> {
  let iam = req.iam();
  iam.check_admin_access()?;

  let db = req.data::<Database>().unwrap();
  let (start, limit) = pagination(&req);
  let maybe_search = search(&req);

  let maybe_github_id = maybe_search.and_then(|search| search.parse().ok());
  let maybe_sort = sort(&req);

  let (total, packages) = db
    .list_packages(start, limit, maybe_search, maybe_github_id, maybe_sort)
    .await?;
  Ok(ApiList {
    items: packages.into_iter().map(|package| package.into()).collect(),
    total,
  })
}

/// Re-runs the package analysis for a published version and stores the
/// freshly computed score meta, which is otherwise only computed at publish
/// time. Docs, the npm tarball, and provenance are left untouched.
#[instrument(
  name = "POST /api/admin/packages/:scope/:package/:version/recompute_meta",
  skip(req),
  fields(scope, package, version)
)]
pub async fn recompute_package_version_meta(
  req: Request<Body>,
) -> ApiResult<ApiPackageScore> {
  let iam = req.iam();
  let staff = iam.check_admin_access()?;
  let staff_id = staff.id;

  let scope = req.param_scope()?;
  let package = req.param_package()?;
  let version = req.param_version()?;
  Span::current().record("scope", field::display(&scope));
  Span::current().record("package", field::display(&package));
  Span::current().record("version", field::display(&version));

  let db = req.data::<Database>().unwrap();
  let buckets = req.data::<Buckets>().unwrap();
  let registry_url = req.data::<RegistryUrl>().unwrap().0.clone();

  let (pkg, _, _) = db
    .get_package(&scope, &package)
    .await?
    .ok_or(ApiError::PackageNotFound)?;
  let package_version = db
    .get_package_version(&scope, &package, &version)
    .await?
    .ok_or(ApiError::PackageVersionNotFound)?;

  let mut files = HashMap::new();
  for file in db.list_package_files(&scope, &package, &version).await? {
    let s3_path =
      crate::s3_paths::file_path(&scope, &package, &version, &file.path);
    let bytes = buckets
      .modules_bucket
      .download(s3_path.into())
      .await?
      .ok_or_else(|| {
        tracing::error!(
          "module file '{}' of @{}/{}@{} is missing from the modules bucket",
          file.path,
          scope,
          package,
          version
        );
        ApiError::InternalServerError
      })?;
    files.insert(file.path, bytes.to_vec());
  }

  // the config file path is only used in analysis error messages; the
  // exports driving the analysis come from the database
  let config_file = ["/jsr.json", "/jsr.jsonc", "/deno.json", "/deno.jsonc"]
    .into_iter()
    .map(|path| PackagePath::new(path.to_string()).unwrap())
    .find(|path| files.contains_key(path))
    .unwrap_or_else(|| PackagePath::new("/jsr.json".to_string()).unwrap());

  let span = Span::current();
  let analysis_scope = scope.clone();
  let analysis_package = package.clone();
  let analysis_version = version.clone();
  let data = PackageAnalysisData {
    exports: package_version.exports.clone(),
    files,
  };
  let output = tokio::task::spawn_blocking(move || {
    analyze_package(
      span,
      registry_url,
      analysis_scope,
      analysis_package,
      analysis_version,
      config_file,
      data,
    )
  })
  .await
  .map_err(|join_error| {
    tracing::error!("analysis task panicked: {join_error:?}");
    ApiError::InternalServerError
  })?
  .map_err(|analysis_error| ApiError::PackageAnalysisFailed {
    msg: analysis_error.to_string(),
  })?;

  let mut meta = output.meta;
  // provenance is recorded post-publish and not derivable from the files
  meta.has_provenance = package_version.meta.has_provenance;

  db.update_package_version_meta(&staff_id, &scope, &package, &version, &meta)
    .await?;

  Ok(ApiPackageScore::from((&meta, &pkg)))
}

#[instrument(name = "GET /api/admin/publishing_tasks", skip(req))]
pub async fn list_publishing_tasks(
  req: Request<Body>,
) -> ApiResult<ApiList<ApiPublishingTask>> {
  let iam = req.iam();
  iam.check_admin_access()?;

  let db = req.data::<Database>().unwrap();
  let (start, limit) = pagination(&req);
  let maybe_search = search(&req);
  let maybe_sort = sort(&req);

  let (total, publishing_tasks) = db
    .list_publishing_tasks(start, limit, maybe_search, maybe_sort)
    .await?;

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
  fields(publishing_task)
)]
pub async fn requeue_publishing_tasks(req: Request<Body>) -> ApiResult<()> {
  let iam = req.iam();
  let staff = iam.check_admin_access()?;

  let publishing_task_id = req.param_uuid("publishing_task")?;
  Span::current()
    .record("publishing_task", field::display(&publishing_task_id));

  let db = req.data::<Database>().unwrap().clone();
  let task = db
    .get_publishing_task(publishing_task_id)
    .await?
    .ok_or(ApiError::PublishNotFound)?;

  if task.0.status == PublishingTaskStatus::Processing {
    db.update_publishing_task_status(
      Some(&staff.id),
      publishing_task_id,
      PublishingTaskStatus::Processing,
      PublishingTaskStatus::Pending,
      None,
    )
    .await?;
  }

  let publish_queue = req.data::<PublishQueue>().unwrap().0.clone();
  let algolia_client = req.data::<Option<AlgoliaClient>>().unwrap().clone();

  if let Some(queue) = publish_queue {
    let body = serde_json::to_vec(&publishing_task_id)?;
    queue.task_buffer(None, Some(body.into())).await?;
  } else {
    let buckets = req.data::<Buckets>().unwrap().clone();
    let license_store = req.data::<LicenseStore>().unwrap().clone();
    let registry = req.data::<RegistryUrl>().unwrap().0.clone();
    let npm_url = req.data::<NpmUrl>().unwrap().0.clone();
    // Re-runs against the fallback registry configured *now*, which may differ
    // from the one the task originally resolved its dependencies against. That
    // is intentional — the requeue re-resolves from scratch — but it means a
    // requeue can record different `dependency_fallback_url`s than the original
    // run did.
    let fallback_registry_url =
      req.data::<FallbackRegistryUrl>().unwrap().0.clone();
    let cache_purge = req
      .data::<crate::external::cloudflare::CachePurge>()
      .unwrap()
      .clone();

    let span = Span::current();
    let fut = publish_task(
      publishing_task_id,
      0,
      buckets,
      license_store,
      registry,
      npm_url,
      fallback_registry_url,
      db,
      algolia_client,
      cache_purge,
    )
    .instrument(span);
    tokio::spawn(fut);
  }

  Ok(())
}

#[instrument(name = "GET /api/admin/tickets", skip(req))]
pub async fn list_tickets(req: Request<Body>) -> ApiResult<ApiList<ApiTicket>> {
  let iam = req.iam();
  iam.check_admin_access()?;

  let db = req.data::<Database>().unwrap();
  let (start, limit) = pagination(&req);
  let maybe_search = search(&req);
  let maybe_sort = sort(&req);

  // An unrecognised value narrows to nothing rather than being ignored, so a
  // typo in the filter cannot silently show the unfiltered queue.
  let maybe_status = match req.query("status").map(String::as_str) {
    None | Some("") => None,
    Some(status) => Some(status.parse::<TicketStatus>().map_err(|_| {
      ApiError::MalformedRequest {
        msg: "unknown ticket status filter".into(),
      }
    })?),
  };

  let (total, tickets) = db
    .list_tickets(start, limit, maybe_search, maybe_sort, maybe_status)
    .await?;
  Ok(ApiList {
    // This endpoint is behind check_admin_access, so notes are included.
    items: tickets
      .into_iter()
      .map(|ticket| ApiTicket::for_viewer(ticket, true))
      .collect(),
    total,
  })
}

#[instrument(name = "PATCH /api/admin/tickets/:id", skip(req))]
pub async fn patch_ticket(mut req: Request<Body>) -> ApiResult<ApiTicket> {
  let id = req.param_uuid("id")?;
  Span::current().record("id", field::display(id));

  let ApiAdminUpdateTicketRequest { status } = decode_json(&mut req).await?;

  let iam = req.iam();
  let staff = iam.check_admin_access()?;

  let db = req.data::<Database>().unwrap();

  let ticket = if let Some(status) = status {
    db.update_ticket_status(&staff.id, id, status).await?
  } else {
    return Err(ApiError::MalformedRequest {
      msg: "missing 'status' parameter".into(),
    });
  };

  Ok(ApiTicket::for_viewer(ticket, true))
}

#[instrument(name = "GET /api/admin/audit_logs", skip(req))]
pub async fn list_audit_logs(
  req: Request<Body>,
) -> ApiResult<ApiList<ApiAuditLog>> {
  let iam = req.iam();
  iam.check_admin_access()?;

  let db = req.data::<Database>().unwrap();
  let (start, limit) = pagination(&req);
  let maybe_search = search(&req);
  let maybe_sort = sort(&req);
  let sudo_only = req.query("sudoOnly").is_some();

  let (total, audit_logs) = db
    .list_audit_logs(start, limit, maybe_search, maybe_sort, sudo_only)
    .await?;
  Ok(ApiList {
    items: audit_logs
      .into_iter()
      .map(|audit_log| audit_log.into())
      .collect(),
    total,
  })
}

#[cfg(test)]
mod tests {
  use crate::api::ApiFullScope;
  use crate::api::ApiFullUser;
  use crate::api::ApiList;
  use crate::api::ApiScope;
  use crate::util::test::ApiResultExt;
  use crate::util::test::TestSetup;
  use hyper::StatusCode;
  use serde_json::json;

  #[tokio::test]
  async fn create_outreach_ticket() {
    use crate::api::ApiTicket;
    use crate::api::ApiTicketActor;
    use crate::api::ApiTicketMessage;
    use crate::db::TicketKind;
    use crate::db::TicketMessageDirection;
    use crate::db::TicketStatus;

    let mut t = TestSetup::new().await;
    let staff_token = t.staff_user.token.clone();
    let user_token = t.user1.token.clone();
    let user_id = t.user1.user.id;
    let path = format!("/api/admin/users/{user_id}/tickets");

    // Not for regular users.
    t.http()
      .post(&path)
      .token(Some(&user_token))
      .body_json(json!({ "subject": "Hi", "message": "hello" }))
      .call()
      .await
      .unwrap()
      .expect_err_code(StatusCode::FORBIDDEN, "actorNotAuthorized")
      .await;

    // An empty message opens nothing.
    t.http()
      .post(&path)
      .token(Some(&staff_token))
      .body_json(json!({ "subject": "Hi", "message": "  " }))
      .call()
      .await
      .unwrap()
      .expect_err_code(StatusCode::BAD_REQUEST, "ticketMessageEmpty")
      .await;

    // Nor does one addressed to nobody.
    t.http()
      .post(format!("/api/admin/users/{}/tickets", uuid::Uuid::new_v4()))
      .token(Some(&staff_token))
      .body_json(json!({ "subject": "Hi", "message": "hello" }))
      .call()
      .await
      .unwrap()
      .expect_err_code(StatusCode::NOT_FOUND, "userNotFound")
      .await;

    let ticket = t
      .http()
      .post(&path)
      .token(Some(&staff_token))
      .body_json(json!({
        "subject": "About your scope",
        "message": "hello from staff",
        "meta": { "scope": "scope" },
      }))
      .call()
      .await
      .unwrap()
      .expect_ok::<ApiTicket>()
      .await;

    assert_eq!(ticket.kind, TicketKind::StaffOutreach);
    assert_eq!(ticket.subject.as_deref(), Some("About your scope"));
    assert_eq!(ticket.status, TicketStatus::WaitingOnUser);
    assert_eq!(ticket.meta, json!({ "scope": "scope" }));
    let ApiTicketActor::User { user: reporter } = &ticket.reporter else {
      panic!("expected a user reporter, got {:?}", ticket.reporter);
    };
    assert_eq!(reporter.id, user_id);
    assert_eq!(ticket.messages.len(), 1);
    assert_eq!(ticket.messages[0].message, "hello from staff");
    assert_eq!(
      ticket.messages[0].direction,
      TicketMessageDirection::Outbound
    );
    let ApiTicketActor::User { user: author } = &ticket.messages[0].author
    else {
      panic!(
        "expected a user author, got {:?}",
        ticket.messages[0].author
      );
    };
    assert_eq!(author.id, t.staff_user.user.id);

    // The contacted user can see it and reply, and their reply is inbound
    // like on any ticket they opened themselves.
    let message = t
      .http()
      .post(format!("/api/tickets/{}", ticket.id))
      .token(Some(&user_token))
      .body_json(json!({ "message": "hello back" }))
      .call()
      .await
      .unwrap()
      .expect_ok::<ApiTicketMessage>()
      .await;
    assert_eq!(message.direction, TicketMessageDirection::Inbound);

    let tickets = t
      .http()
      .get("/api/user/tickets")
      .token(Some(&user_token))
      .call()
      .await
      .unwrap()
      .expect_ok::<Vec<ApiTicket>>()
      .await;
    assert_eq!(tickets.len(), 1);
    assert_eq!(tickets[0].id, ticket.id);
    assert_eq!(tickets[0].status, TicketStatus::WaitingOnSupport);

    // Somebody else cannot.
    let other_token = t.user2.token.clone();
    t.http()
      .get(format!("/api/tickets/{}", ticket.id))
      .token(Some(&other_token))
      .call()
      .await
      .unwrap()
      .expect_err_code(StatusCode::NOT_FOUND, "ticketNotFound")
      .await;
  }

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

  #[tokio::test]
  async fn assign_scope() {
    let mut t = TestSetup::new().await;

    // create a scope for a user2
    let path = "/api/admin/scopes";
    let token = t.staff_user.token.clone();
    let user2_id = t.user2.user.id;
    let scope = t
      .http()
      .post(path)
      .body_json(json!({
        "scope": "test-scope",
        "userId": user2_id,
      }))
      .token(Some(&token))
      .call()
      .await
      .unwrap()
      .expect_ok::<ApiScope>()
      .await;
    assert_eq!(scope.scope.to_string(), "test-scope");

    // create a scope with a reserved name
    let res = t
      .http()
      .post(path)
      .body_json(json!({
        "scope": "react",
        "userId": user2_id,
      }))
      .token(Some(&token))
      .call()
      .await
      .unwrap()
      .expect_ok::<ApiScope>()
      .await;
    assert_eq!(res.scope.to_string(), "react");

    // create a scope with an existing name
    t.http()
      .post(path)
      .body_json(json!({
        "scope": "test-scope",
        "userId": user2_id,
      }))
      .token(Some(&token))
      .call()
      .await
      .unwrap()
      .expect_err_code(StatusCode::CONFLICT, "scopeAlreadyExists")
      .await;
  }

  #[tokio::test]
  async fn recompute_package_version_meta() {
    use std::io::Write as _;

    use bytes::Bytes;
    use flate2::Compression;
    use flate2::write::GzEncoder;

    use crate::api::ApiPackageScore;
    use crate::db::PublishingTaskStatus;
    use crate::ids::PackageName;
    use crate::ids::ScopeName;
    use crate::ids::Version;

    let mut t = TestSetup::new().await;

    // a JS entrypoint typed via `/// <reference types>`, the layout from
    // jsr-io/jsr#698
    let mut tar_bytes = Vec::new();
    let mut tar = tar::Builder::new(&mut tar_bytes);
    let mut append = |path: &str, content: &[u8]| {
      let mut header = tar::Header::new_gnu();
      header.set_size(content.len() as u64);
      header.set_mode(0o644);
      header.set_cksum();
      tar.append_data(&mut header, path, content).unwrap();
    };
    append(
      "jsr.json",
      br#"{ "name": "@scope/foo", "version": "1.2.3", "license": "MIT", "exports": "./mod.mjs" }"#,
    );
    append("README.md", b"# foo\n\n```ts\nadd(1, 2);\n```\n");
    append(
      "mod.mjs",
      b"/// <reference types=\"./mod.d.ts\" />\n/** Adds two numbers. */\nexport function add(a, b) {\n  return a + b;\n}\n",
    );
    append(
      "mod.d.ts",
      b"/** A module. @module */\n\n/** Adds two numbers. */\nexport declare function add(a: number, b: number): number;\n",
    );
    tar.finish().unwrap();
    drop(tar);
    let mut gz_bytes = Vec::new();
    let mut encoder = GzEncoder::new(&mut gz_bytes, Compression::default());
    encoder.write_all(&tar_bytes).unwrap();
    encoder.finish().unwrap();

    let task =
      crate::publish::tests::process_tarball_setup(&t, Bytes::from(gz_bytes))
        .await;
    assert_eq!(
      task.status,
      PublishingTaskStatus::Success,
      "{:?}",
      task.error
    );

    let scope = ScopeName::try_from("scope").unwrap();
    let name = PackageName::try_from("foo").unwrap();
    let version = Version::try_from("1.2.3").unwrap();

    let fresh = t
      .db()
      .get_package_version(&scope, &name, &version)
      .await
      .unwrap()
      .unwrap();
    assert!(fresh.meta.all_fast_check);
    assert!(fresh.meta.has_readme);

    // simulate stale meta from an older analysis pipeline
    let mut stale = fresh.meta.clone();
    stale.all_fast_check = false;
    stale.has_readme = false;
    stale.has_provenance = true;
    t.db()
      .update_package_version_meta(
        &t.staff_user.user.id,
        &scope,
        &name,
        &version,
        &stale,
      )
      .await
      .unwrap();

    let path = "/api/admin/packages/scope/foo/1.2.3/recompute_meta";

    let user_token = t.user1.token.clone();
    t.http()
      .post(path)
      .token(Some(&user_token))
      .call()
      .await
      .unwrap()
      .expect_err_code(StatusCode::FORBIDDEN, "actorNotAuthorized")
      .await;

    let staff_token = t.staff_user.token.clone();
    let score = t
      .http()
      .post(path)
      .token(Some(&staff_token))
      .call()
      .await
      .unwrap()
      .expect_ok::<ApiPackageScore>()
      .await;
    assert!(score.all_fast_check);
    assert!(score.has_readme);
    assert!(score.has_provenance);

    let healed = t
      .db()
      .get_package_version(&scope, &name, &version)
      .await
      .unwrap()
      .unwrap();
    assert!(healed.meta.all_fast_check);
    assert!(healed.meta.has_readme);
    assert!(healed.meta.has_provenance);
  }
}
