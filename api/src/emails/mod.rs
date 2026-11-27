// Copyright 2024 the JSR authors. All rights reserved. MIT license.
use std::borrow::Cow;
use std::sync::OnceLock;

use handlebars::Handlebars;
use serde::Serialize;

pub use shared::emails::*;
use shared::ids::ScopeName;

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
const SUPPORT_TICKET_OUTREACH_TXT: &str = "support_ticket_outreach.txt";
const SUPPORT_TICKET_OUTREACH_HTML: &str = "support_ticket_outreach.html";

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
  /// Sent to a user when staff open a ticket addressed to them. Unlike the
  /// other ticket emails it is not a reply to anything, so it introduces the
  /// conversation rather than announcing a new message on it.
  SupportTicketOutreach {
    name: Cow<'a, str>,
    ticket_id: Cow<'a, str>,
    ticket_number: Cow<'a, str>,
    subject: Cow<'a, str>,
    content: Cow<'a, str>,
    registry_url: Cow<'a, str>,
    registry_name: Cow<'a, str>,
    support_email: Cow<'a, str>,
  },
}

impl EmailArgs<'_> {
  fn text_template_filename(&self) -> &'static str {
    match self {
      EmailArgs::ScopeInvite { .. } => SCOPE_INVITE_TXT,
      EmailArgs::PersonalAccessToken { .. } => PERSONAL_ACCESS_TOKEN_TXT,
      EmailArgs::SupportTicketCreated { .. } => SUPPORT_TICKET_CREATED_TXT,
      EmailArgs::SupportTicketMessage { .. } => SUPPORT_TICKET_MESSAGE_TXT,
      EmailArgs::SupportTicketAutoReply { .. } => SUPPORT_TICKET_AUTO_REPLY_TXT,
      EmailArgs::SupportTicketOutreach { .. } => SUPPORT_TICKET_OUTREACH_TXT,
    }
  }

  fn html_template_filename(&self) -> &'static str {
    match self {
      EmailArgs::ScopeInvite { .. } => SCOPE_INVITE_HTML,
      EmailArgs::PersonalAccessToken { .. } => PERSONAL_ACCESS_TOKEN_HTML,
      EmailArgs::SupportTicketCreated { .. } => SUPPORT_TICKET_CREATED_HTML,
      EmailArgs::SupportTicketMessage { .. } => SUPPORT_TICKET_MESSAGE_HTML,
      EmailArgs::SupportTicketAutoReply { .. } => {
        SUPPORT_TICKET_AUTO_REPLY_HTML
      }
      EmailArgs::SupportTicketOutreach { .. } => SUPPORT_TICKET_OUTREACH_HTML,
    }
  }
}

impl Email for EmailArgs<'_> {
  fn subject(&self) -> String {
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
      EmailArgs::SupportTicketOutreach {
        ticket_number,
        subject,
        ..
      } => {
        format!("[{ticket_number}] {}", subject.trim())
      }
    }
  }

  fn render(&self) -> Result<EmailContent, anyhow::Error> {
    static TEMPLATE_ENGINE: OnceLock<Handlebars<'static>> = OnceLock::new();
    let hbs = TEMPLATE_ENGINE.get_or_init(|| init_handlebars().unwrap());
    Ok(EmailContent {
      text: hbs.render(self.text_template_filename(), self)?,
      html: hbs.render(self.html_template_filename(), self)?,
    })
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
  t.register_template_string(
    SUPPORT_TICKET_OUTREACH_TXT,
    include_str!("./templates/support_ticket_outreach.txt.hbs"),
  )?;
  t.register_template_string(
    SUPPORT_TICKET_OUTREACH_HTML,
    include_str!("./templates/support_ticket_outreach.html.hbs"),
  )?;

  t.set_strict_mode(true);

  Ok(t)
}
