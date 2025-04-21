// Copyright 2024 the JSR authors. All rights reserved. MIT license.

use hyper::Body;
use hyper::Request;
use routerify::prelude::RequestExt;
use routerify::Router;
use std::borrow::Cow;
use tracing::field;
use tracing::instrument;
use tracing::Span;

use crate::db::NewTicket;
use crate::db::NewTicketMessage;
use crate::db::{Database, UserPublic};
use crate::emails::EmailArgs;
use crate::emails::EmailSender;
use crate::iam::ReqIamExt;
use crate::util;
use crate::util::decode_json;
use crate::util::ApiResult;
use crate::util::RequestIdExt;
use crate::RegistryUrl;

use super::ApiError;
use super::ApiTicket;
use super::ApiTicketMessage;

pub fn tickets_router() -> Router<Body, ApiError> {
  Router::builder()
    .post("/", util::auth(util::json(post_handler)))
    .get("/:id", util::auth(util::json(get_handler)))
    .post("/:id", util::auth(util::json(post_message_handler)))
    .build()
    .unwrap()
}

#[instrument(name = "GET /api/tickets/:id", skip(req), err, fields(id))]
pub async fn get_handler(req: Request<Body>) -> ApiResult<ApiTicket> {
  let id = req.param_uuid("id")?;
  Span::current().record("id", field::display(id));

  let db = req.data::<Database>().unwrap();
  let ticket = db.get_ticket(id).await?.ok_or(ApiError::TicketNotFound)?;

  let iam = req.iam();

  let current_user = iam.check_current_user_access()?;
  if current_user == &ticket.1 || iam.check_admin_access().is_ok() {
    Ok(ticket.into())
  } else {
    Err(ApiError::TicketNotFound)
  }
}

#[instrument(name = "POST /api/tickets", skip(req), err)]
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
        registry_url: Cow::Borrowed(registry_url.0.as_str()),
        registry_name: Cow::Borrowed(&email_sender.from_name),
        support_email: Cow::Borrowed(&email_sender.from),
      };
      email_sender
        .send(email.clone(), email_args)
        .await
        .map_err(|e| {
          tracing::error!("failed to send email: {:?}", e);
          ApiError::InternalServerError
        })?;
    }
  }

  Ok(
    (
      ticket,
      user.clone(),
      vec![(message, UserPublic::from(user))],
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

  let (ticket, creator, _) =
    db.get_ticket(id).await?.ok_or(ApiError::TicketNotFound)?;

  let iam = req.iam();

  let current_user = iam.check_current_user_access()?;
  if !(current_user == &creator || iam.check_admin_access().is_ok()) {
    return Err(ApiError::TicketNotFound);
  }

  let (message, message_author) = db
    .ticket_add_message(id, current_user.id, new_message)
    .await?;

  // only send email to ticket creator if the message was not sent by ticket creator
  if creator.id != message_author.id {
    if let Some(email) = &creator.email {
      let email_sender = req.data::<Option<EmailSender>>().unwrap();
      let registry_url = req.data::<RegistryUrl>().unwrap();
      if let Some(email_sender) = email_sender {
        let email_args = EmailArgs::SupportTicketMessage {
          ticket_id: Cow::Owned(ticket.id.to_string()),
          name: Cow::Owned(creator.name),
          content: Cow::Borrowed(&message.message),
          registry_url: Cow::Borrowed(registry_url.0.as_str()),
          registry_name: Cow::Borrowed(&email_sender.from_name),
          support_email: Cow::Borrowed(&email_sender.from),
        };
        email_sender
          .send(email.clone(), email_args)
          .await
          .map_err(|e| {
            tracing::error!("failed to send email: {:?}", e);
            ApiError::InternalServerError
          })?;
      }
    }
  }

  Ok((message, message_author).into())
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

    assert_eq!(ticket.creator.id, user_id);
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
    let ticket: ApiTicket = resp.expect_ok().await;
    assert_eq!(ticket.messages[0].message, "test");
    assert_eq!(ticket.messages[1].message, "test2");

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
    let _ticket: ApiTicket = resp.expect_ok().await;
  }
}
