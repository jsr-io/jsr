// Copyright 2024 the JSR authors. All rights reserved. MIT license.

//! Posts support ticket activity to a Discord channel through an incoming
//! webhook, so staff hear about new tickets and replies without having to watch
//! the admin panel.

use serde_json::Value;
use serde_json::json;
use std::time::Duration;
use tracing::error;
use tracing::instrument;
use url::Url;

use crate::db::Ticket;
use crate::db::TicketKind;

/// How long a webhook call may hold up the request that triggered it.
const TIMEOUT: Duration = Duration::from_secs(5);

/// The message is only a preview; staff read the full text on the ticket page.
const MAX_PREVIEW_CHARS: usize = 1000;

/// Discord rejects a field value above 1024 characters, and escaping can double
/// a string's length.
const MAX_FIELD_CHARS: usize = 250;

/// Discord rejects an embed title above 256 characters.
const MAX_TITLE_CHARS: usize = 120;

const COLOR_OPENED: u32 = 0x57f287;
const COLOR_REPLIED: u32 = 0x5865f2;

#[derive(Clone)]
pub struct DiscordWebhookClient {
  url: String,
}

/// Wrapper around an optional `DiscordWebhookClient` so it can be stored in the
/// router data alongside other shared services. A `None` value means no webhook
/// was configured (local dev, tests), and notifications are skipped.
#[derive(Clone)]
pub struct SupportDiscord(pub Option<DiscordWebhookClient>);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TicketEvent {
  Opened,
  Replied,
}

impl SupportDiscord {
  /// Tell the support channel about a new ticket, or a reply from the person on
  /// the other side of one.
  ///
  /// Best-effort, like cache purging: the ticket is already saved, so a Discord
  /// outage is logged rather than failing the request (or, for inbound email,
  /// making Postmark redeliver a message we have already ingested).
  pub async fn notify(
    &self,
    registry_url: &Url,
    ticket: &Ticket,
    event: TicketEvent,
    from: &str,
    message: &str,
  ) {
    let Some(client) = &self.0 else {
      return;
    };
    let payload = ticket_payload(registry_url, ticket, event, from, message);
    if let Err(err) = client.execute(&payload).await {
      error!("failed to post support ticket to Discord: {err:#}");
    }
  }
}

impl DiscordWebhookClient {
  pub fn new(url: String) -> Self {
    Self { url }
  }

  #[instrument(name = "discord.execute_webhook", skip_all, err)]
  async fn execute(&self, payload: &Value) -> Result<(), anyhow::Error> {
    let response = crate::util::shared_http_client()
      .post(&self.url)
      .timeout(TIMEOUT)
      .json(payload)
      .send()
      .await?;

    if !response.status().is_success() {
      let status = response.status();
      let body = response.text().await.unwrap_or_default();
      return Err(anyhow::anyhow!(
        "Discord webhook failed (status={status}): {body}"
      ));
    }

    Ok(())
  }
}

fn ticket_payload(
  registry_url: &Url,
  ticket: &Ticket,
  event: TicketEvent,
  from: &str,
  message: &str,
) -> Value {
  let summary = ticket
    .subject
    .as_deref()
    .unwrap_or_else(|| kind_label(&ticket.kind));
  let (verb, color) = match event {
    TicketEvent::Opened => ("New ticket", COLOR_OPENED),
    TicketEvent::Replied => ("New reply on", COLOR_REPLIED),
  };

  json!({
    // Everything below is written by whoever opened the ticket. Without this,
    // an `@everyone` in a support request would ping the whole server.
    "allowed_mentions": { "parse": [] },
    "embeds": [{
      "title": format!(
        "{verb} {}: {}",
        ticket.ticket_number,
        escape_markdown(&truncate(summary, MAX_TITLE_CHARS)),
      ),
      "url": format!("{registry_url}ticket/{}", ticket.id),
      "description": escape_markdown(&truncate(message, MAX_PREVIEW_CHARS)),
      "color": color,
      "fields": [
        {
          "name": "From",
          "value": escape_markdown(&truncate(from, MAX_FIELD_CHARS)),
          "inline": true,
        },
        {
          "name": "Kind",
          "value": kind_label(&ticket.kind),
          "inline": true,
        },
      ],
      "timestamp": ticket.updated_at.to_rfc3339(),
    }],
  })
}

fn kind_label(kind: &TicketKind) -> &'static str {
  match kind {
    TicketKind::UserScopeQuotaIncrease => "User scope quota increase",
    TicketKind::ScopeQuotaIncrease => "Scope quota increase",
    TicketKind::ScopeClaim => "Scope claim",
    TicketKind::PackageReport => "Package report",
    TicketKind::Other => "Other",
    TicketKind::StaffOutreach => "Staff outreach",
  }
}

fn truncate(s: &str, max_chars: usize) -> String {
  let s = s.trim();
  match s.char_indices().nth(max_chars) {
    Some((end, _)) => format!("{}…", &s[..end]),
    None => s.to_owned(),
  }
}

/// Neutralizes Discord markdown in text a stranger wrote. Masked links in
/// particular would otherwise let a ticket show staff one URL and link another.
fn escape_markdown(s: &str) -> String {
  let mut out = String::with_capacity(s.len());
  for c in s.chars() {
    if matches!(
      c,
      '\\'
        | '*'
        | '_'
        | '~'
        | '`'
        | '|'
        | '>'
        | '<'
        | '['
        | ']'
        | '('
        | ')'
        | '#'
        | '-'
    ) {
      out.push('\\');
    }
    out.push(c);
  }
  out
}

#[cfg(test)]
mod test {
  use super::*;
  use crate::db::TicketStatus;
  use uuid::Uuid;

  fn ticket(subject: Option<&str>) -> Ticket {
    Ticket {
      id: Uuid::nil(),
      ticket_number: "TICKET-20261015-ABCDE".to_owned(),
      kind: TicketKind::Other,
      creator: None,
      reporter_email: None,
      reporter_name: None,
      subject: subject.map(str::to_owned),
      claim_token: None,
      meta: json!({}),
      status: TicketStatus::Open,
      closed_at: None,
      updated_at: chrono::Utc::now(),
      created_at: chrono::Utc::now(),
    }
  }

  #[test]
  fn payload_links_ticket_and_disables_mentions() {
    let registry_url: Url = "https://jsr.io/".parse().unwrap();
    let payload = ticket_payload(
      &registry_url,
      &ticket(Some("Help")),
      TicketEvent::Opened,
      "someone@example.com",
      "@everyone please help",
    );

    assert_eq!(payload["allowed_mentions"], json!({ "parse": [] }));
    let embed = &payload["embeds"][0];
    assert_eq!(embed["title"], "New ticket TICKET-20261015-ABCDE: Help");
    assert_eq!(
      embed["url"],
      "https://jsr.io/ticket/00000000-0000-0000-0000-000000000000"
    );
    assert_eq!(embed["description"], "@everyone please help");
  }

  #[test]
  fn payload_falls_back_to_kind_without_subject() {
    let registry_url: Url = "https://jsr.io/".parse().unwrap();
    let payload = ticket_payload(
      &registry_url,
      &ticket(None),
      TicketEvent::Replied,
      "Someone",
      "thanks",
    );
    assert_eq!(
      payload["embeds"][0]["title"],
      "New reply on TICKET-20261015-ABCDE: Other"
    );
  }

  #[test]
  fn escapes_masked_links() {
    assert_eq!(
      escape_markdown("[jsr.io](https://evil.example)"),
      r"\[jsr.io\]\(https://evil.example\)"
    );
  }

  #[test]
  fn truncates_on_char_boundary() {
    assert_eq!(truncate("héllo", 2), "hé…");
    assert_eq!(truncate("  hi  ", 10), "hi");
  }
}
