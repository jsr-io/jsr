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
    .post("/", util::json(post_handler))
    .get("/:id", util::json(get_handler))
    .post("/:id", util::json(post_message_handler))
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

  if let Some(meta) = &new_ticket.meta {
    if !meta.is_object() {
      return Err(ApiError::TicketMetaNotValid);
    }
  }

  let (ticket, user, message) = db.create_ticket(new_ticket).await?;

  if let Some(email) = &user.email {
    let email_sender = req.data::<Option<EmailSender>>().unwrap();
    let registry_url = req.data::<RegistryUrl>().unwrap();
    if let Some(email_sender) = email_sender {
      let email_args = EmailArgs::SupportTicketCreated {
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
