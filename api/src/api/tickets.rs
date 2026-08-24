// Copyright 2024 the JSR authors. All rights reserved. MIT license.

use hyper::Body;
use hyper::Request;
use hyper::Response;
use hyper::StatusCode;
use routerify::Router;
use routerify::prelude::RequestExt;
use routerify_query::RequestQueryExt;
use std::borrow::Cow;
use tracing::Span;
use tracing::field;
use tracing::instrument;
use uuid::Uuid;

use crate::RegistryUrl;
use crate::db::FullTicket;
use crate::db::FullTicketMessage;
use crate::db::NewTicket;
use crate::db::NewTicketMessage;
use crate::db::Ticket;
use crate::db::{Database, UserPublic};
use crate::emails;
use crate::emails::EmailArgs;
use crate::emails::EmailQueue;
use crate::emails::EmailSender;
use crate::emails::EmailThread;
use crate::iam::ReqIamExt;
use crate::s3::Buckets;
use crate::util;
use crate::util::ApiResult;
use crate::util::RequestIdExt;
use crate::util::decode_json;

use super::ApiError;
use super::ApiTicket;
use super::ApiTicketMessage;
use super::ApiTicketMessageOrAuditLog;
use super::ApiTicketOverview;

pub fn tickets_router() -> Router<Body, ApiError> {
  Router::builder()
    .post("/", util::auth(util::json(post_handler)))
    .get("/:id", util::auth(util::json(get_handler)))
    .post("/:id", util::auth(util::json(post_message_handler)))
    .post("/:id/claim", util::auth(util::json(claim_handler)))
    .get(
      "/:id/attachments/:attachment",
      util::auth(get_attachment_handler),
    )
    .build()
    .unwrap()
}

/// How the caller proved they are allowed to see a ticket.
enum TicketAccess {
  /// A signed-in account: the ticket's owner, or a staff member.
  User,
  /// The reporter of an email-opened ticket nobody has claimed yet, holding the
  /// claim token from the auto-reply. They can read and reply, but the ticket is
  /// not theirs until they claim it.
  Reporter,
}

/// Decides whether the request may see `ticket`, and on what basis.
///
/// A claim token in the query string stands in for a session, so that whoever
/// emailed support can follow the ticket on the web before (or without) making
/// an account. It is checked first: holding the token is proof enough on its
/// own, and a signed-in visitor following the link out of their inbox is the
/// expected case, not an exception.
fn check_ticket_access(
  req: &Request<Body>,
  ticket: &Ticket,
) -> Result<TicketAccess, ApiError> {
  if let Some(claim_token) = ticket.claim_token
    && let Some(provided) = req.query("claim")
    && Uuid::parse_str(provided) == Ok(claim_token)
  {
    return Ok(TicketAccess::Reporter);
  }

  let iam = req.iam();
  let current_user = iam.check_current_user_access()?;
  if ticket.creator == Some(current_user.id) || iam.check_admin_access().is_ok()
  {
    return Ok(TicketAccess::User);
  }

  // Deliberately not "forbidden": that would confirm the ticket exists to
  // anyone guessing IDs.
  Err(ApiError::TicketNotFound)
}

/// Where a new outbound email slots into a ticket's email thread.
///
/// `References` lists the whole conversation so far, oldest first, so clients
/// can group the thread even if one message never reached them.
fn thread_for<'a>(
  messages: &[FullTicketMessage],
  message_id: &'a str,
) -> EmailThread<'a> {
  let references: Vec<String> = messages
    .iter()
    .filter_map(|(message, ..)| message.email_message_id.clone())
    .collect();

  EmailThread {
    message_id,
    in_reply_to: references.last().cloned(),
    references,
  }
}

/// Generates a `Message-ID` for an email JSR is about to send. Recorded against
/// the message it announces, so a reply pointing back at it can be threaded onto
/// the right ticket.
fn new_email_message_id(registry_url: &RegistryUrl) -> String {
  let domain = registry_url.0.host_str().unwrap_or("jsr.io");
  format!("<{}@{}>", Uuid::new_v4(), domain)
}

#[instrument(name = "GET /api/tickets/:id", skip(req), fields(id))]
pub async fn get_handler(req: Request<Body>) -> ApiResult<ApiTicketOverview> {
  let id = req.param_uuid("id")?;

  Span::current().record("id", field::display(id));

  let db = req.data::<Database>().unwrap();

  let (ticket, creator, messages) =
    db.get_ticket(id).await?.ok_or(ApiError::TicketNotFound)?;

  check_ticket_access(&req, &ticket)?;

  let mut events: Vec<ApiTicketMessageOrAuditLog> = messages
    .into_iter()
    .map(|message| ApiTicketMessageOrAuditLog::Message {
      message: message.into(),
    })
    .collect();

  if let Ok(audit_logs) = db.get_ticket_audit_logs(id).await {
    for (audit_log, user) in audit_logs {
      events.push(ApiTicketMessageOrAuditLog::AuditLog { audit_log, user });
    }
  }

  events.sort_by_key(|event| match event {
    ApiTicketMessageOrAuditLog::Message { message, .. } => message.created_at,
    ApiTicketMessageOrAuditLog::AuditLog { audit_log, .. } => {
      audit_log.created_at
    }
  });

  Ok((ticket, creator, events).into())
}

#[instrument(name = "POST /api/tickets", skip(req))]
pub async fn post_handler(mut req: Request<Body>) -> ApiResult<ApiTicket> {
  let new_ticket: NewTicket = decode_json(&mut req).await?;
  let db = req.data::<Database>().unwrap();

  let iam = req.iam();
  let user = iam.check_current_user_access()?;

  if !new_ticket.meta.is_object() {
    return Err(ApiError::TicketMetaNotValid);
  }

  let (ticket, user, message) = db.create_ticket(user.id, new_ticket).await?;

  if let Some(email) = &user.email {
    let email_sender = req.data::<Option<EmailSender>>().unwrap();
    let registry_url = req.data::<RegistryUrl>().unwrap();
    if let Some(email_sender) = email_sender {
      let email_args = EmailArgs::SupportTicketCreated {
        name: Cow::Borrowed(&user.name),
        ticket_id: Cow::Owned(ticket.id.to_string()),
        ticket_number: Cow::Borrowed(&ticket.ticket_number),
        registry_url: Cow::Borrowed(registry_url.0.as_str()),
        registry_name: Cow::Borrowed(&email_sender.from_name),
        support_email: Cow::Borrowed(&email_sender.from),
      };
      // Queued without a Message-ID of its own: there is nothing yet for a
      // reply to thread onto, so a reply to this acknowledgement is matched by
      // the ticket number in its subject instead. Every later email in the
      // thread does carry one.
      if let Err(err) = emails::enqueue(
        db,
        email_sender,
        req.data::<EmailQueue>().unwrap(),
        email.clone(),
        email_args,
        None,
      )
      .await
      {
        tracing::error!("failed to queue email: {:?}", err);
      }
    }
  }

  Ok(
    (
      ticket,
      Some(user.clone()),
      vec![(message, Some(UserPublic::from(user)), vec![])],
    )
      .into(),
  )
}

#[instrument(name = "POST /api/tickets/:id", skip(req), fields(id))]
pub async fn post_message_handler(
  mut req: Request<Body>,
) -> ApiResult<ApiTicketMessage> {
  let id = req.param_uuid("id")?;
  Span::current().record("id", field::display(id));

  let new_message: NewTicketMessage = decode_json(&mut req).await?;
  let db = req.data::<Database>().unwrap();

  let (ticket, creator, messages) =
    db.get_ticket(id).await?.ok_or(ApiError::TicketNotFound)?;

  let access = check_ticket_access(&req, &ticket)?;

  if new_message.message.is_empty() {
    return Err(ApiError::TicketMessageEmpty);
  }

  let email_sender = req.data::<Option<EmailSender>>().unwrap();
  let registry_url = req.data::<RegistryUrl>().unwrap();

  // Generated before the insert so the stored row and the header on the email
  // announcing it agree, which is what makes the reply threadable.
  let email_message_id = email_sender
    .as_ref()
    .map(|_| new_email_message_id(registry_url));

  // Who should be told about this message by email: the person on the other side
  // of the conversation, if it wasn't them who just wrote it.
  let notify: Option<(String, String)>;

  let message = match access {
    TicketAccess::Reporter => {
      let reporter_email = ticket
        .reporter_email
        .clone()
        .ok_or(ApiError::TicketNotFound)?;
      // The reporter writing to themselves needs no email.
      notify = None;
      let message = db
        .ticket_add_reporter_message(
          id,
          &reporter_email,
          ticket.reporter_name.as_deref(),
          None,
          new_message,
        )
        .await?;
      (message, None, vec![])
    }
    TicketAccess::User => {
      let iam = req.iam();
      let author = iam.check_current_user_access()?;
      notify = match &creator {
        // A message from staff on somebody's own ticket.
        Some(creator) if creator.id != author.id => creator
          .email
          .clone()
          .map(|email| (email, creator.name.clone())),
        // A message on an unclaimed, email-opened ticket: staff replying to
        // whoever wrote in.
        None => ticket.reporter_email.clone().map(|email| {
          let name = ticket
            .reporter_name
            .clone()
            .unwrap_or_else(|| email.clone());
          (email, name)
        }),
        // The ticket's owner talking; nothing to notify them of.
        Some(_) => None,
      };

      let (message, user) = db
        .ticket_add_message(
          id,
          author.id,
          // Only recorded when an email actually goes out under it; a
          // Message-ID for a message nobody was sent would thread nothing.
          notify
            .is_some()
            .then_some(email_message_id.as_deref())
            .flatten(),
          new_message,
        )
        .await?;
      (message, Some(user), vec![])
    }
  };

  if let Some((email, name)) = notify
    && let Some(email_sender) = email_sender
    && let Some(email_message_id) = &email_message_id
  {
    let email_args = EmailArgs::SupportTicketMessage {
      ticket_id: Cow::Owned(ticket.id.to_string()),
      ticket_number: Cow::Borrowed(&ticket.ticket_number),
      name: Cow::Owned(name),
      content: Cow::Borrowed(&message.0.message),
      registry_url: Cow::Borrowed(registry_url.0.as_str()),
      registry_name: Cow::Borrowed(&email_sender.from_name),
      support_email: Cow::Borrowed(&email_sender.from),
    };
    // A failure here is logged rather than returned: the message is already on
    // the ticket, and failing the request would have the admin retype a reply
    // that was in fact saved. The sweeper re-drives anything left unsent.
    if let Err(err) = emails::enqueue(
      db,
      email_sender,
      req.data::<EmailQueue>().unwrap(),
      email,
      email_args,
      Some(thread_for(&messages, email_message_id)),
    )
    .await
    {
      tracing::error!("failed to queue email: {:?}", err);
    }
  }

  Ok(message.into())
}

/// Binds an email-opened ticket to the signed-in account, so it shows up
/// alongside their other tickets instead of only in their inbox.
#[instrument(name = "POST /api/tickets/:id/claim", skip(req), fields(id))]
pub async fn claim_handler(req: Request<Body>) -> ApiResult<ApiTicket> {
  let id = req.param_uuid("id")?;
  Span::current().record("id", field::display(id));

  let iam = req.iam();
  let user = iam.check_current_user_access()?;

  let claim_token = req
    .query("claim")
    .and_then(|token| Uuid::parse_str(token).ok())
    .ok_or(ApiError::TicketClaimTokenInvalid)?;

  let db = req.data::<Database>().unwrap();
  let ticket: FullTicket = db
    .claim_ticket(id, claim_token, user.id)
    .await?
    .ok_or(ApiError::TicketClaimTokenInvalid)?;

  Ok(ticket.into())
}

#[instrument(
  name = "GET /api/tickets/:id/attachments/:attachment",
  skip(req),
  fields(id, attachment)
)]
pub async fn get_attachment_handler(
  req: Request<Body>,
) -> ApiResult<Response<Body>> {
  let id = req.param_uuid("id")?;
  let attachment_id = req.param_uuid("attachment")?;
  Span::current().record("id", field::display(id));
  Span::current().record("attachment", field::display(attachment_id));

  let db = req.data::<Database>().unwrap();

  let (ticket, ..) =
    db.get_ticket(id).await?.ok_or(ApiError::TicketNotFound)?;
  check_ticket_access(&req, &ticket)?;

  let attachment = db
    .get_ticket_attachment(id, attachment_id)
    .await?
    .ok_or(ApiError::TicketAttachmentNotFound)?;

  let buckets = req.data::<Buckets>().unwrap();
  let bytes = buckets
    .ticket_attachments_bucket
    .download(attachment.storage_key.into())
    .await?
    .ok_or(ApiError::TicketAttachmentNotFound)?;

  let mut res = util::create_response(
    StatusCode::OK,
    // Attachments are files a stranger emailed us. Serving them under their
    // claimed content type would let an HTML or SVG "attachment" run script on
    // the API origin, so they are always handed over as opaque bytes to save.
    "application/octet-stream",
    bytes,
  );
  res.headers_mut().insert(
    hyper::header::CONTENT_DISPOSITION,
    content_disposition_attachment(&attachment.filename)
      .parse()
      .unwrap(),
  );
  Ok(res)
}

/// Builds a `Content-Disposition` header that asks the browser to save the file
/// under `filename`.
///
/// The filename came from an email a stranger sent, so the plain `filename=`
/// form is reduced to something that cannot break out of its quotes, with the
/// real name carried in RFC 5987 `filename*` for clients that understand it.
fn content_disposition_attachment(filename: &str) -> String {
  let fallback: String = filename
    .chars()
    .map(|c| {
      if c.is_ascii_graphic() && c != '"' && c != '\\' {
        c
      } else {
        '_'
      }
    })
    .collect();
  let encoded = percent_encoding::utf8_percent_encode(
    filename,
    percent_encoding::NON_ALPHANUMERIC,
  );
  format!("attachment; filename=\"{fallback}\"; filename*=UTF-8''{encoded}")
}

#[cfg(test)]
mod test {
  use crate::api::ApiTicket;
  use crate::api::ApiTicketMessage;
  use crate::db::TicketKind;
  use crate::util::test::ApiResultExt;
  use crate::util::test::TestSetup;
  use hyper::StatusCode;
  use serde_json::json;

  #[tokio::test]
  async fn test_ticket() {
    let mut t = TestSetup::new().await;

    let user_id = t.user1.user.id;
    let user_token = t.user1.token.clone();
    let mut resp = t
      .http()
      .post("/api/tickets")
      .token(Some(&user_token))
      .body_json(json!({
        "kind": TicketKind::UserScopeQuotaIncrease,
        "meta": {},
        "message": "test".to_string(),
      }))
      .call()
      .await
      .unwrap();
    let ticket: ApiTicket = resp.expect_ok().await;

    let super::super::ApiTicketActor::User { user: reporter } =
      &ticket.reporter
    else {
      panic!("expected a user reporter, got {:?}", ticket.reporter);
    };
    assert_eq!(reporter.id, user_id);
    assert_eq!(ticket.messages[0].message, "test");

    let mut resp = t
      .http()
      .post(format!("/api/tickets/{}", ticket.id))
      .token(Some(&user_token))
      .body_json(json!({
        "message": "test2".to_string(),
      }))
      .call()
      .await
      .unwrap();
    let message: ApiTicketMessage = resp.expect_ok().await;
    assert_eq!(message.message, "test2");

    let mut resp = t
      .http()
      .get(format!("/api/tickets/{}", ticket.id))
      .token(Some(&user_token))
      .call()
      .await
      .unwrap();
    let ticket_overview: super::ApiTicketOverview = resp.expect_ok().await;

    let mut message_contents: Vec<String> = Vec::new();
    for event in &ticket_overview.events {
      if let super::ApiTicketMessageOrAuditLog::Message { message, .. } = event
      {
        message_contents.push(message.message.clone());
      }
    }
    assert!(
      message_contents.len() >= 2,
      "Expected at least 2 messages, found {}",
      message_contents.len()
    );
    assert_eq!(message_contents[0], "test");
    assert_eq!(message_contents[1], "test2");

    let other_user_token = t.user2.token.clone();
    let mut resp = t
      .http()
      .get(format!("/api/tickets/{}", ticket.id))
      .token(Some(&other_user_token))
      .call()
      .await
      .unwrap();
    resp.expect_err(StatusCode::NOT_FOUND).await;

    let staff_user_token = t.staff_user.token.clone();
    let mut resp = t
      .http()
      .get(format!("/api/tickets/{}", ticket.id))
      .token(Some(&staff_user_token))
      .call()
      .await
      .unwrap();
    let staff_ticket_overview: super::ApiTicketOverview =
      resp.expect_ok().await;

    let mut staff_message_contents: Vec<String> = Vec::new();
    for event in &staff_ticket_overview.events {
      if let super::ApiTicketMessageOrAuditLog::Message { message, .. } = event
      {
        staff_message_contents.push(message.message.clone());
      }
    }
    assert!(
      staff_message_contents.len() >= 2,
      "Expected at least 2 messages for staff view, found {}",
      staff_message_contents.len()
    );
    assert_eq!(staff_message_contents[0], "test");
    assert_eq!(staff_message_contents[1], "test2");
  }
}
