// Copyright 2024 the JSR authors. All rights reserved. MIT license.
use std::borrow::Cow;
use std::sync::OnceLock;

use handlebars::Handlebars;
use handlebars::RenderError;
use postmark::Query;
use postmark::reqwest::PostmarkClient;
use serde::Serialize;
use tracing::instrument;

use crate::ids::ScopeName;

const BASE_TXT: &str = "base.txt";
const BASE_HTML: &str = "base.html";
const SCOPE_INVITE_TXT: &str = "scope_invite.txt";
const SCOPE_INVITE_HTML: &str = "scope_invite.html";
const PERSONAL_ACCESS_TOKEN_TXT: &str = "personal_access_token.txt";
const PERSONAL_ACCESS_TOKEN_HTML: &str = "personal_access_token.html";
const SUPPORT_TICKET_CREATED_TXT: &str = "support_ticket_created.txt";
const SUPPORT_TICKET_CREATED_HTML: &str = "support_ticket_created.html";
const SUPPORT_TICKET_MESSAGE_TXT: &str = "support_ticket_message.txt";
const SUPPORT_TICKET_MESSAGE_HTML: &str = "support_ticket_message.html";
const SUPPORT_TICKET_AUTO_REPLY_TXT: &str = "support_ticket_auto_reply.txt";
const SUPPORT_TICKET_AUTO_REPLY_HTML: &str = "support_ticket_auto_reply.html";

#[derive(Debug, Serialize)]
#[serde(untagged)]
pub enum EmailArgs<'a> {
  ScopeInvite {
    name: Cow<'a, str>,
    inviter_name: Cow<'a, str>,
    scope: Cow<'a, ScopeName>,
    registry_url: Cow<'a, str>,
    registry_name: Cow<'a, str>,
    support_email: Cow<'a, str>,
  },
  PersonalAccessToken {
    token_description: Cow<'a, str>,
    token_permissions: Cow<'a, str>,
    token_expiry: Cow<'a, str>,
    name: Cow<'a, str>,
    registry_url: Cow<'a, str>,
    registry_name: Cow<'a, str>,
    support_email: Cow<'a, str>,
  },
  SupportTicketCreated {
    name: Cow<'a, str>,
    ticket_id: Cow<'a, str>,
    ticket_number: Cow<'a, str>,
    registry_url: Cow<'a, str>,
    registry_name: Cow<'a, str>,
    support_email: Cow<'a, str>,
  },
  SupportTicketMessage {
    name: Cow<'a, str>,
    ticket_id: Cow<'a, str>,
    ticket_number: Cow<'a, str>,
    content: Cow<'a, str>,
    registry_url: Cow<'a, str>,
    registry_name: Cow<'a, str>,
    support_email: Cow<'a, str>,
  },
  /// Sent back to whoever emailed support, acknowledging that their mail opened
  /// a ticket and handing them the link that binds it to an account.
  SupportTicketAutoReply {
    /// The display name from the sender's `From` header, if it had one.
    name: Option<Cow<'a, str>>,
    ticket_number: Cow<'a, str>,
    original_subject: Cow<'a, str>,
    claim_url: Cow<'a, str>,
    registry_url: Cow<'a, str>,
    registry_name: Cow<'a, str>,
    support_email: Cow<'a, str>,
  },
}

impl EmailArgs<'_> {
  pub fn subject(&self) -> String {
    match self {
      EmailArgs::ScopeInvite {
        scope,
        registry_name,
        ..
      } => {
        format!("You've been invited to @{scope} on {registry_name}")
      }
      EmailArgs::PersonalAccessToken { registry_name, .. } => {
        format!("A new personal access token was created on {registry_name}")
      }
      // The ticket number goes in the subject of every ticket email so that a
      // reply which arrives with no usable threading headers can still be
      // matched back to its ticket. See `api/src/api/hooks.rs`.
      EmailArgs::SupportTicketCreated { ticket_number, .. }
      | EmailArgs::SupportTicketMessage { ticket_number, .. } => {
        format!("[{ticket_number}] Support request")
      }
      EmailArgs::SupportTicketAutoReply {
        ticket_number,
        original_subject,
        ..
      } => {
        // Keep the reporter's own subject so the exchange still reads as one
        // thread in their mail client, without stacking up `Re:` prefixes.
        let subject = original_subject
          .trim()
          .strip_prefix("Re:")
          .unwrap_or(original_subject)
          .trim();
        format!("[{ticket_number}] Re: {subject}")
      }
    }
  }

  pub fn text_template_filename(&self) -> &'static str {
    match self {
      EmailArgs::ScopeInvite { .. } => SCOPE_INVITE_TXT,
      EmailArgs::PersonalAccessToken { .. } => PERSONAL_ACCESS_TOKEN_TXT,
      EmailArgs::SupportTicketCreated { .. } => SUPPORT_TICKET_CREATED_TXT,
      EmailArgs::SupportTicketMessage { .. } => SUPPORT_TICKET_MESSAGE_TXT,
      EmailArgs::SupportTicketAutoReply { .. } => SUPPORT_TICKET_AUTO_REPLY_TXT,
    }
  }

  pub fn html_template_filename(&self) -> &'static str {
    match self {
      EmailArgs::ScopeInvite { .. } => SCOPE_INVITE_HTML,
      EmailArgs::PersonalAccessToken { .. } => PERSONAL_ACCESS_TOKEN_HTML,
      EmailArgs::SupportTicketCreated { .. } => SUPPORT_TICKET_CREATED_HTML,
      EmailArgs::SupportTicketMessage { .. } => SUPPORT_TICKET_MESSAGE_HTML,
      EmailArgs::SupportTicketAutoReply { .. } => {
        SUPPORT_TICKET_AUTO_REPLY_HTML
      }
    }
  }
}

fn init_handlebars()
-> Result<Handlebars<'static>, Box<handlebars::TemplateError>> {
  let mut t: Handlebars<'static> = Handlebars::new();

  t.register_template_string(
    BASE_TXT,
    include_str!("./templates/base.txt.hbs"),
  )?;
  t.register_template_string(
    BASE_HTML,
    include_str!("./templates/base.html.hbs"),
  )?;
  t.register_template_string(
    SCOPE_INVITE_TXT,
    include_str!("./templates/scope_invite.txt.hbs"),
  )?;
  t.register_template_string(
    SCOPE_INVITE_HTML,
    include_str!("./templates/scope_invite.html.hbs"),
  )?;
  t.register_template_string(
    PERSONAL_ACCESS_TOKEN_TXT,
    include_str!("./templates/personal_access_token.txt.hbs"),
  )?;
  t.register_template_string(
    PERSONAL_ACCESS_TOKEN_HTML,
    include_str!("./templates/personal_access_token.html.hbs"),
  )?;
  t.register_template_string(
    SUPPORT_TICKET_CREATED_TXT,
    include_str!("./templates/support_ticket_created.txt.hbs"),
  )?;
  t.register_template_string(
    SUPPORT_TICKET_CREATED_HTML,
    include_str!("./templates/support_ticket_created.html.hbs"),
  )?;
  t.register_template_string(
    SUPPORT_TICKET_MESSAGE_TXT,
    include_str!("./templates/support_ticket_message.txt.hbs"),
  )?;
  t.register_template_string(
    SUPPORT_TICKET_MESSAGE_HTML,
    include_str!("./templates/support_ticket_message.html.hbs"),
  )?;
  t.register_template_string(
    SUPPORT_TICKET_AUTO_REPLY_TXT,
    include_str!("./templates/support_ticket_auto_reply.txt.hbs"),
  )?;
  t.register_template_string(
    SUPPORT_TICKET_AUTO_REPLY_HTML,
    include_str!("./templates/support_ticket_auto_reply.html.hbs"),
  )?;

  t.set_strict_mode(true);

  Ok(t)
}

#[derive(Debug)]
pub struct EmailContent {
  pub text: String,
  pub html: String,
}

pub fn email_content(args: EmailArgs) -> Result<EmailContent, RenderError> {
  static TEMPLATE_ENGINE: OnceLock<Handlebars<'static>> = OnceLock::new();
  let hbs = TEMPLATE_ENGINE.get_or_init(|| init_handlebars().unwrap());

  let text_filename = args.text_template_filename();
  let html_filename = args.html_template_filename();

  let text = hbs.render(text_filename, &args)?;
  let html = hbs.render(html_filename, &args)?;

  Ok(EmailContent { text, html })
}

/// Where an outgoing email sits in an email thread. All three fields hold RFC
/// 5322 `Message-ID`s, angle brackets included.
#[derive(Debug)]
pub struct EmailThread<'a> {
  /// The `Message-ID` to send under. Callers generate this before sending and
  /// record it, so that a reply pointing back at it can be matched to the
  /// conversation it belongs to.
  pub message_id: &'a str,
  /// The message being replied to, if any.
  pub in_reply_to: Option<String>,
  /// The conversation so far, oldest first. Mail clients use this to group the
  /// thread even when an intermediate message is missing.
  pub references: Vec<String>,
}

fn header(name: &str, value: &str) -> postmark::api::email::Header {
  postmark::api::email::Header {
    name: name.to_owned(),
    value: value.to_owned(),
  }
}

#[derive(Debug)]
pub struct EmailSender {
  postmark: PostmarkClient,
  pub from: String,
  pub from_name: String,
}

impl EmailSender {
  pub fn new(
    postmark: PostmarkClient,
    from: String,
    from_name: String,
  ) -> Self {
    Self {
      postmark,
      from,
      from_name,
    }
  }

  /// Sends an already-rendered email. Callers reach this through
  /// [`enqueue`] rather than directly, so that a Postmark failure retries out
  /// of band instead of failing the request that triggered the mail.
  pub async fn send_rendered(
    &self,
    to: String,
    subject: String,
    text: String,
    html: String,
    thread: Option<EmailThread<'_>>,
  ) -> Result<(), anyhow::Error> {
    let mut request = postmark::api::email::SendEmailRequest::builder()
      .from(format!("{} <{}>", self.from_name, self.from))
      .to(to)
      .subject(subject)
      .body(postmark::api::Body::HtmlAndText { html, text })
      .build();

    if let Some(thread) = thread {
      let mut headers = vec![header("Message-ID", thread.message_id)];
      if let Some(in_reply_to) = &thread.in_reply_to {
        headers.push(header("In-Reply-To", in_reply_to));
      }
      if !thread.references.is_empty() {
        headers.push(header("References", &thread.references.join(" ")));
      }
      request.headers = Some(headers);
    }

    let resp = request.execute(&self.postmark).await?;
    if resp.error_code != 0 {
      Err(anyhow::anyhow!(
        "Postmark error {}: {}",
        resp.error_code,
        resp.message
      ))
    } else {
      Ok(())
    }
  }
}

/// The Cloud Tasks queue that drives `/tasks/send_email`. `None` outside
/// production, where deliveries are sent inline instead (see [`enqueue`]).
pub struct EmailQueue(pub Option<crate::gcp::Queue>);

/// How many times a delivery is attempted before it is abandoned. Cloud Tasks
/// does its own retrying on top of this; the count here is the backstop that
/// stops a permanently undeliverable address being retried forever.
pub const MAX_EMAIL_ATTEMPTS: i32 = 8;

/// Queues an email and asks Cloud Tasks to deliver it.
///
/// The row is committed first and the queue is nudged afterwards, so a failure
/// to reach Cloud Tasks costs a delay rather than the email: the sweeper in
/// `tasks.rs` re-drives anything that never reached a terminal state. Errors
/// from the nudge are logged and swallowed for that reason — the caller's own
/// request must not fail because of an email.
///
/// With no queue configured (local development, tests) the delivery is sent
/// inline so behaviour matches production as closely as it can without Cloud
/// Tasks.
#[instrument(name = "emails::enqueue", skip_all, err, fields(delivery_id))]
pub async fn enqueue(
  db: &crate::db::Database,
  email_sender: &EmailSender,
  queue: &EmailQueue,
  to: String,
  args: EmailArgs<'_>,
  thread: Option<EmailThread<'_>>,
) -> Result<uuid::Uuid, anyhow::Error> {
  let subject = args.subject();
  let content = email_content(args)?;

  let (message_id, in_reply_to, reference_ids) = match thread {
    Some(thread) => (
      Some(thread.message_id.to_owned()),
      thread.in_reply_to,
      thread.references,
    ),
    None => (None, None, Vec::new()),
  };

  let id = db
    .enqueue_email(crate::db::NewEmailDelivery {
      to_address: to,
      subject,
      body_text: content.text,
      body_html: content.html,
      message_id,
      in_reply_to,
      reference_ids,
    })
    .await?;

  tracing::Span::current().record("delivery_id", tracing::field::display(id));

  match &queue.0 {
    Some(queue) => {
      let body = serde_json::to_vec(&SendEmailTask { id })?;
      if let Err(err) = queue
        .task_buffer(Some(id.to_string()), Some(body.into()))
        .await
      {
        tracing::error!(
          delivery_id = %id,
          "failed to enqueue email delivery task, leaving it for the sweeper: {:?}",
          err
        );
      }
    }
    None => {
      if let Err(err) = deliver(db, Some(email_sender), id).await {
        tracing::error!(
          delivery_id = %id,
          "failed to send email inline: {:?}",
          err
        );
      }
    }
  }

  Ok(id)
}

/// The body of a `/tasks/send_email` request.
#[derive(Debug, serde::Serialize, serde::Deserialize)]
pub struct SendEmailTask {
  pub id: uuid::Uuid,
}

/// Attempts one delivery, recording the outcome against the row.
///
/// Returns `Ok(true)` when the delivery reached a terminal state (sent, already
/// sent, or abandoned) and must not be retried, and `Ok(false)` when it failed
/// but is worth another attempt. The caller turns the latter into a non-2xx so
/// Cloud Tasks retries.
#[instrument(name = "emails::deliver", skip(db, email_sender), err)]
pub async fn deliver(
  db: &crate::db::Database,
  email_sender: Option<&EmailSender>,
  id: uuid::Uuid,
) -> Result<bool, anyhow::Error> {
  let Some(delivery) = db.get_pending_email_delivery(id).await? else {
    // Already sent or abandoned. Cloud Tasks redelivering a task it has already
    // run must not produce a second email.
    return Ok(true);
  };

  // A delivery can reach a deployment that has no Postmark credential — most
  // easily by the queue dispatching to a service that was never given one.
  // Recorded as an ordinary failure rather than swallowed, so it retries if the
  // configuration is fixed, and is eventually abandoned with a readable reason
  // instead of disappearing.
  let Some(email_sender) = email_sender else {
    let abandoned = db
      .record_email_delivery_failure(
        id,
        "no email sender is configured on this service",
        MAX_EMAIL_ATTEMPTS,
      )
      .await?;
    tracing::error!(
      delivery_id = %id,
      to = %delivery.to_address,
      abandoned,
      "cannot deliver email: no email sender is configured on this service"
    );
    return Ok(abandoned);
  };

  let thread = delivery
    .message_id
    .as_deref()
    .map(|message_id| EmailThread {
      message_id,
      in_reply_to: delivery.in_reply_to.clone(),
      references: delivery.reference_ids.clone(),
    });

  match email_sender
    .send_rendered(
      delivery.to_address.clone(),
      delivery.subject.clone(),
      delivery.body_text.clone(),
      delivery.body_html.clone(),
      thread,
    )
    .await
  {
    Ok(()) => {
      db.mark_email_delivery_sent(id).await?;
      Ok(true)
    }
    Err(err) => {
      let abandoned = db
        .record_email_delivery_failure(
          id,
          &format!("{err:?}"),
          MAX_EMAIL_ATTEMPTS,
        )
        .await?;
      if abandoned {
        tracing::error!(
          delivery_id = %id,
          to = %delivery.to_address,
          "giving up on email delivery after {} attempts: {:?}",
          MAX_EMAIL_ATTEMPTS,
          err
        );
      } else {
        tracing::warn!(delivery_id = %id, "email delivery failed, will retry: {:?}", err);
      }
      Ok(abandoned)
    }
  }
}
