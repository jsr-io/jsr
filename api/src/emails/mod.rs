// Copyright 2024 the JSR authors. All rights reserved. MIT license.
use std::borrow::Cow;
use std::sync::OnceLock;

use handlebars::Handlebars;
use handlebars::RenderError;
use postmark::Query;
use postmark::reqwest::PostmarkClient;
use serde::Serialize;

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

  pub async fn send(
    &self,
    to: String,
    args: EmailArgs<'_>,
  ) -> Result<(), anyhow::Error> {
    self.send_threaded(to, args, None).await
  }

  /// Sends an email that is part of an ongoing conversation, carrying the RFC
  /// 5322 headers that let the recipient's reply be threaded back onto the
  /// ticket it belongs to.
  pub async fn send_threaded(
    &self,
    to: String,
    args: EmailArgs<'_>,
    thread: Option<EmailThread<'_>>,
  ) -> Result<(), anyhow::Error> {
    let subject = args.subject();
    let content = email_content(args)?;
    let mut request = postmark::api::email::SendEmailRequest::builder()
      .from(format!("{} <{}>", self.from_name, self.from))
      .to(to)
      .subject(subject)
      .body(postmark::api::Body::HtmlAndText {
        html: content.html,
        text: content.text,
      })
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
