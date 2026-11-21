// Copyright 2024 the JSR authors. All rights reserved. MIT license.

//! Inbound email. Cloudflare Email Routing hands every message sent to the
//! domain to the API worker's `email` handler, which passes the raw message to
//! [`ingest`]; each one either opens a support ticket or is appended to the
//! ticket it is replying to.

use bytes::Bytes;
use std::borrow::Cow;
use std::sync::LazyLock;
use tracing::Span;
use tracing::field;
use tracing::instrument;
use url::Url;
use uuid::Uuid;

use crate::emails;
use crate::emails::EmailArgs;
use crate::emails::EmailQueue;
use crate::emails::EmailSender;
use crate::emails::EmailThread;
use crate::s3::Buckets;
use crate::s3::S3UploadOptions;
use crate::s3::UploadTaskBody;
use shared::db::Database;
use shared::models::NewEmailTicket;
use shared::models::NewTicketAttachment;
use shared::models::NewTicketEmailMessage;
use shared::models::NewTicketSystemMessage;

use super::api::ApiError;

/// Files above this are dropped rather than stored. Email Routing caps an
/// inbound message at 25 MiB in total; this is well under that, and comfortably
/// above anything a support conversation legitimately needs.
const MAX_ATTACHMENT_BYTES: usize = 10 * 1024 * 1024;

/// Guards against one message filling the bucket with many small files.
const MAX_ATTACHMENTS_PER_MESSAGE: usize = 20;

/// Stored message bodies are capped so that a runaway email cannot bloat a row
/// without bound. Anything longer is truncated with a marker.
const MAX_BODY_BYTES: usize = 256 * 1024;

/// What [`ingest`] needs from the running service.
pub struct InboundContext<'a> {
  pub db: &'a Database,
  pub buckets: &'a Buckets,
  pub registry_url: &'a Url,
  pub email_sender: Option<&'a EmailSender>,
  pub email_queue: &'a EmailQueue,
  /// The `authserv-id` of the mail server that hands us inbound mail —
  /// `mx.cloudflare.net` for Email Routing.
  ///
  /// That server checks SPF and DKIM against the real sender and records the
  /// outcome in an `Authentication-Results` header, which is what decides
  /// whether a sender's address counts as proven. Only the header written by
  /// the server named here is believed, since anyone can write one claiming
  /// whatever they like.
  ///
  /// `None` means no server is trusted, and every sender is recorded as
  /// unverified.
  pub trusted_authserv_id: Option<&'a str>,
}

/// Why an inbound email was not ingested.
#[derive(Debug)]
// Only the Worker's handler reads the reasons.
#[cfg_attr(not(target_arch = "wasm32"), allow(dead_code))]
pub enum InboundError {
  /// The message itself is unacceptable and would be on every retry, so the
  /// sending server is told not to try again.
  Rejected(&'static str),
  /// Something failed on our side. The message is fine, and the sending server
  /// should deliver it again later.
  Failed(ApiError),
}

/// The parts of an inbound email we act on.
#[derive(Debug)]
struct InboundEmail {
  from: InboundAddress,
  subject: Option<String>,
  text_body: Option<String>,
  /// In the order they appear in the message, so the ones added by the servers
  /// that handled it come before the ones the sender wrote.
  headers: Vec<InboundHeader>,
  attachments: Vec<InboundAttachment>,
}

#[derive(Debug)]
struct InboundAddress {
  email: String,
  name: Option<String>,
}

#[derive(Debug)]
struct InboundHeader {
  name: String,
  value: String,
}

#[derive(Debug)]
struct InboundAttachment {
  name: String,
  content: Bytes,
  content_type: String,
}

impl InboundEmail {
  /// Reads a raw RFC 5322 message. `None` if it is not one, or names no sender.
  ///
  /// Attachments that are too large or too numerous are left out with a
  /// warning rather than failing the delivery — losing an attachment is better
  /// than losing the support request it came with.
  fn parse(raw: &[u8]) -> Option<Self> {
    let message = mail_parser::MessageParser::default().parse(raw)?;

    let sender = message.from()?.first()?;
    let from = InboundAddress {
      email: sender.address()?.to_owned(),
      name: sender.name().map(str::to_owned),
    };

    let headers = message
      .headers()
      .iter()
      .map(|header| InboundHeader {
        name: header.raw_name().to_owned(),
        // A long header is folded across lines; the breaks are not part of
        // its value.
        value: String::from_utf8_lossy(header.raw_value())
          .split_whitespace()
          .collect::<Vec<_>>()
          .join(" "),
      })
      .collect();

    let parts = message.attachments();
    if parts.len() > MAX_ATTACHMENTS_PER_MESSAGE {
      tracing::warn!(
        total = parts.len(),
        kept = MAX_ATTACHMENTS_PER_MESSAGE,
        "inbound email had more attachments than we store"
      );
    }
    let mut attachments = Vec::new();
    for part in parts.take(MAX_ATTACHMENTS_PER_MESSAGE) {
      let name = part.attachment_name().unwrap_or("attachment").to_owned();
      // Measured before decoding, so an oversized file is never held in
      // memory a second time.
      let size = part.decoded_len();
      if size > MAX_ATTACHMENT_BYTES {
        tracing::warn!(
          filename = %name,
          size,
          "skipping oversized inbound attachment"
        );
        continue;
      }
      let content_type = match part.content_type() {
        Some(content_type) => match content_type.subtype() {
          Some(subtype) => format!("{}/{subtype}", content_type.ctype()),
          None => content_type.ctype().to_owned(),
        },
        None => "application/octet-stream".to_owned(),
      };
      attachments.push(InboundAttachment {
        name,
        content: Bytes::from(part.decoded().into_owned()),
        content_type,
      });
    }

    Some(InboundEmail {
      from,
      subject: message.subject().map(str::to_owned),
      // An HTML-only message is converted to text.
      text_body: message.body_text(0).map(Cow::into_owned),
      headers,
      attachments,
    })
  }

  fn header(&self, name: &str) -> Option<&str> {
    self
      .headers
      .iter()
      .find(|header| header.name.eq_ignore_ascii_case(name))
      .map(|header| header.value.as_str())
  }

  /// The sending domain's own `Message-ID`, angle brackets included. Without
  /// one there is nothing to deduplicate on, so such a message is refused
  /// rather than risk being ingested twice.
  fn message_id(&self) -> Option<&str> {
    self.header("Message-ID").filter(|id| !id.is_empty())
  }

  /// Whether the sending domain passed both SPF and DKIM, according to the
  /// trusted mail server.
  ///
  /// Unverified mail still opens a ticket — a misconfigured sender with a real
  /// problem should not be silently dropped — but the flag is recorded and
  /// surfaced so nobody treats the address as proven.
  fn is_verified(&self, trusted_authserv_id: Option<&str>) -> bool {
    let Some(results) = trusted_authserv_id
      .and_then(|trusted| self.trusted_auth_results(trusted))
    else {
      return false;
    };
    method_result(&results, "spf").as_deref() == Some("pass")
      && method_result(&results, "dkim").as_deref() == Some("pass")
  }

  /// The `Authentication-Results` written by the trusted server, if it is
  /// present.
  ///
  /// Only the first matching header is considered. Each server that handles a
  /// message prepends its own headers, so the trusted server's results sit above
  /// anything the sender put in the message themselves — including a header
  /// forged to carry the trusted server's name. Taking any but the first would
  /// let a sender award themselves a pass.
  fn trusted_auth_results(&self, trusted_authserv_id: &str) -> Option<String> {
    self
      .headers
      .iter()
      .filter(|header| {
        header.name.eq_ignore_ascii_case("Authentication-Results")
      })
      .map(|header| strip_comments(&header.value))
      .find(|value| {
        // The authserv-id is everything before the first `;`, optionally
        // followed by the version the server speaks.
        value
          .split(';')
          .next()
          .and_then(|id| id.split_whitespace().next())
          .is_some_and(|id| id.eq_ignore_ascii_case(trusted_authserv_id))
      })
  }

  /// Every `Message-ID` this email claims to be part of, newest first, from the
  /// `In-Reply-To` and `References` headers.
  fn referenced_message_ids(&self) -> Vec<String> {
    let mut ids: Vec<String> = Vec::new();
    for header in ["In-Reply-To", "References"] {
      let Some(value) = self.header(header) else {
        continue;
      };
      // `References` is whitespace-separated per RFC 5322, but some clients use
      // commas; splitting on both costs nothing and handles either.
      for id in value.split([',', ' ', '\t', '\r', '\n']) {
        let id = id.trim();
        if !id.is_empty() && !ids.iter().any(|seen| seen == id) {
          ids.push(id.to_owned());
        }
      }
    }
    ids
  }

  /// The text to store for this message.
  ///
  /// A reply usually quotes the conversation so far below what was written. The
  /// quoted history is cut off, so a long thread is not re-stored in full on
  /// every round trip.
  fn body(&self) -> String {
    let full = self.text_body.as_deref().unwrap_or_default().trim();

    let is_reply = !self.referenced_message_ids().is_empty();
    let body = Some(full)
      .filter(|_| is_reply)
      .map(strip_quoted_history)
      // Nothing left means the reply was written inside or below the quote;
      // keep all of it rather than store an empty message.
      .filter(|stripped| !stripped.is_empty())
      .unwrap_or(full);

    if body.len() <= MAX_BODY_BYTES {
      return body.to_owned();
    }

    // Cut on a character boundary so the result stays valid UTF-8.
    let mut end = MAX_BODY_BYTES;
    while !body.is_char_boundary(end) {
      end -= 1;
    }
    format!("{}\n\n[message truncated]", &body[..end])
  }
}

/// Returns the part of a reply above the history its mail client quoted.
///
/// There is no standard for how a client marks quoted history, so this
/// recognises the common shapes: an "On … wrote:" attribution, Outlook's
/// "Original Message" divider and its `From:` / `Sent:` block, and a trailing
/// run of `>` lines. Anything it does not recognise is kept.
fn strip_quoted_history(body: &str) -> &str {
  let lines: Vec<&str> = body.lines().collect();

  let quote_start = (0..lines.len()).find(|&i| {
    let line = lines[i].trim();

    // "On Mon, 1 Jan 2026 at 10:00, Someone <a@example.com> wrote:", which
    // clients wrap onto a second line when the address is long.
    if line.starts_with("On ") {
      let ends = |l: &&str| l.trim_end().ends_with("wrote:");
      if ends(&line) || lines.get(i + 1).is_some_and(ends) {
        return true;
      }
    }

    if line
      .trim_matches('-')
      .trim()
      .eq_ignore_ascii_case("Original Message")
      && line.starts_with("--")
    {
      return true;
    }

    if line.starts_with("From:")
      && lines[i + 1..]
        .iter()
        .take(3)
        .any(|l| l.trim_start().starts_with("Sent:"))
    {
      return true;
    }

    // A quote with no attribution we recognise, running to the end.
    line.starts_with('>')
      && lines[i..]
        .iter()
        .all(|l| l.trim().is_empty() || l.trim_start().starts_with('>'))
  });

  match quote_start {
    Some(i) => {
      let offset = lines[i].as_ptr() as usize - body.as_ptr() as usize;
      body[..offset].trim_end()
    }
    None => body,
  }
}

/// Removes RFC 5322 comments — parenthesised, possibly nested — from a header
/// value.
///
/// `Authentication-Results` carries human-readable comments that routinely
/// contain both `;` and text like `spf=pass`, so anything that reads the header
/// by splitting or searching has to drop them first or be misled by them.
fn strip_comments(value: &str) -> String {
  let mut out = String::with_capacity(value.len());
  let mut depth = 0usize;
  for c in value.chars() {
    match c {
      '(' => depth += 1,
      ')' => depth = depth.saturating_sub(1),
      _ if depth == 0 => out.push(c),
      _ => {}
    }
  }
  out
}

/// The result recorded for one authentication method in an
/// `Authentication-Results` value, e.g. `pass` from `spf=pass`.
///
/// Expects a value that has already been through [`strip_comments`]. Matches the
/// method name exactly, so `dkim` does not also match `dkim-atps`.
fn method_result(value: &str, method: &str) -> Option<String> {
  // The first segment is the authserv-id, never a method.
  value.split(';').skip(1).find_map(|segment| {
    let token = segment.split_whitespace().next()?;
    let (name, result) = token.split_once('=')?;
    name
      .eq_ignore_ascii_case(method)
      .then(|| result.to_ascii_lowercase())
  })
}

/// Matches the ticket number JSR puts in the subject of every ticket email.
static TICKET_NUMBER_RE: LazyLock<regex::Regex> =
  LazyLock::new(|| regex::Regex::new(r"TICKET-\d{8}-\d{5}").unwrap());

/// Files one inbound email, given as the raw RFC 5322 message.
#[instrument(name = "inbound_email::ingest", skip_all, fields(ticket_id))]
pub async fn ingest(
  ctx: InboundContext<'_>,
  raw: &[u8],
) -> Result<(), InboundError> {
  let Some(mut email) = InboundEmail::parse(raw) else {
    tracing::warn!("rejecting inbound email that could not be read");
    return Err(InboundError::Rejected("message could not be read"));
  };

  let Some(email_message_id) = email.message_id().map(str::to_owned) else {
    // Nothing to deduplicate on, so ingesting this could duplicate the message
    // every time the sending server retries it. Refusing is honest: the
    // message is malformed, and the sender is told not to keep trying.
    tracing::warn!("rejecting inbound email with no Message-ID header");
    return Err(InboundError::Rejected("message has no Message-ID header"));
  };

  async {
    let ticket_id = resolve_ticket(ctx.db, &email).await?;
    if let Some(ticket_id) = ticket_id {
      Span::current().record("ticket_id", field::display(ticket_id));
    }

    // Uploads happen before the row is written so the storage key can name the
    // message it belongs to. A failure later leaves unreferenced objects
    // behind, which is preferable to rows pointing at files that were never
    // stored.
    let message_id = Uuid::new_v4();
    let attachments = store_attachments(
      ctx.buckets,
      message_id,
      std::mem::take(&mut email.attachments),
    )
    .await?;

    let message = NewTicketEmailMessage {
      id: message_id,
      author_email: email.from.email.clone(),
      author_name: email.from.name.clone().filter(|n| !n.is_empty()),
      author_email_verified: email.is_verified(ctx.trusted_authserv_id),
      email_message_id,
      message: email.body(),
      attachments,
    };

    match ticket_id {
      Some(ticket_id) => {
        if ctx
          .db
          .ticket_add_email_message(ticket_id, message)
          .await?
          .is_none()
        {
          tracing::info!("ignoring already-ingested inbound email");
        }
      }
      None => open_ticket(&ctx, message, &email).await?,
    }

    Ok(())
  }
  .await
  .map_err(InboundError::Failed)
}

/// Works out which ticket an inbound email belongs to, if any.
///
/// Threading headers are authoritative when present. The ticket number in the
/// subject is the fallback for clients that drop them, and for someone replying
/// to a notification email that predates threading support.
async fn resolve_ticket(
  db: &Database,
  email: &InboundEmail,
) -> Result<Option<Uuid>, ApiError> {
  let referenced = email.referenced_message_ids();
  if let Some(ticket_id) =
    db.find_ticket_by_email_message_ids(&referenced).await?
  {
    return Ok(Some(ticket_id));
  }

  let Some(subject) = &email.subject else {
    return Ok(None);
  };
  let Some(ticket_number) = TICKET_NUMBER_RE.find(subject) else {
    return Ok(None);
  };

  Ok(db.find_ticket_by_number(ticket_number.as_str()).await?)
}

/// Opens a new ticket for an email that matched nothing, and acknowledges it.
async fn open_ticket(
  ctx: &InboundContext<'_>,
  message: NewTicketEmailMessage,
  email: &InboundEmail,
) -> Result<(), ApiError> {
  let subject = email
    .subject
    .clone()
    .filter(|subject| !subject.trim().is_empty())
    .unwrap_or_else(|| "(no subject)".to_owned());

  let claim_token = Uuid::new_v4();
  let domain = ctx.registry_url.host_str().unwrap_or("jsr.io");
  let auto_reply_email_message_id = format!("<{}@{}>", Uuid::new_v4(), domain);

  let reporter_email = message.author_email.clone();
  let reporter_name = message.author_name.clone();
  let in_reply_to = message.email_message_id.clone();

  let new_ticket = NewEmailTicket {
    reporter_email: reporter_email.clone(),
    reporter_name: reporter_name.clone(),
    subject: subject.clone(),
    claim_token,
    message,
    auto_reply: NewTicketSystemMessage {
      id: Uuid::new_v4(),
      email_message_id: auto_reply_email_message_id.clone(),
      // The email itself is rendered from a template; the ticket timeline just
      // needs to show that an acknowledgement went out.
      message:
        "Automatic reply: we received your email and opened this ticket."
          .to_owned(),
    },
  };

  let Some((ticket, ..)) = ctx.db.create_ticket_from_email(new_ticket).await?
  else {
    tracing::info!("ignoring already-ingested inbound email");
    return Ok(());
  };

  Span::current().record("ticket_id", field::display(ticket.id));

  let Some(email_sender) = ctx.email_sender else {
    return Ok(());
  };

  let claim_url = format!(
    "{}ticket/{}?claim={}",
    ctx.registry_url, ticket.id, claim_token
  );
  let email_args = EmailArgs::SupportTicketAutoReply {
    name: reporter_name.map(Cow::Owned),
    ticket_number: Cow::Borrowed(&ticket.ticket_number),
    original_subject: Cow::Borrowed(&subject),
    claim_url: Cow::Borrowed(&claim_url),
    registry_url: Cow::Borrowed(ctx.registry_url.as_str()),
    registry_name: Cow::Borrowed(&email_sender.from_name),
    support_email: Cow::Borrowed(&email_sender.from),
  };

  // Queued rather than sent here: the ticket is already on file, and a failure
  // to send must not fail the delivery. The sending server would deliver the
  // message again, find it already ingested, and skip past the send entirely —
  // so the reporter would never be acknowledged. The delivery row is retried
  // instead.
  if let Err(err) = emails::enqueue(
    ctx.db,
    email_sender,
    ctx.email_queue,
    reporter_email,
    email_args,
    Some(EmailThread {
      message_id: &auto_reply_email_message_id,
      in_reply_to: Some(in_reply_to.clone()),
      references: vec![in_reply_to],
    }),
  )
  .await
  {
    tracing::error!("failed to queue support ticket auto-reply: {:?}", err);
  }

  Ok(())
}

/// Uploads an email's attachments and returns the rows to record for them.
async fn store_attachments(
  buckets: &Buckets,
  message_id: Uuid,
  attachments: Vec<InboundAttachment>,
) -> Result<Vec<NewTicketAttachment>, ApiError> {
  let mut out = Vec::new();

  for attachment in attachments {
    let attachment_id = Uuid::new_v4();
    let storage_key = format!("tickets/{message_id}/{attachment_id}");
    let size_bytes = attachment.content.len();

    buckets
      .ticket_attachments_bucket
      .upload(
        storage_key.clone().into(),
        UploadTaskBody::Bytes(attachment.content),
        S3UploadOptions {
          // Stored and served as opaque bytes; the sender's claimed type is
          // recorded in the row for display, but never used to serve the file.
          content_type: Some(Cow::Borrowed("application/octet-stream")),
          cache_control: None,
          gzip_encoded: false,
        },
      )
      .await?;

    out.push(NewTicketAttachment {
      filename: attachment.name,
      content_type: attachment.content_type,
      size_bytes: size_bytes as i32,
      storage_key,
    });
  }

  Ok(out)
}

#[cfg(test)]
mod test {
  use super::*;

  fn email(headers: &[(&str, &str)], subject: &str) -> InboundEmail {
    InboundEmail {
      from: InboundAddress {
        email: "someone@example.com".to_owned(),
        name: Some("Someone".to_owned()),
      },
      subject: Some(subject.to_owned()),
      text_body: Some("body".to_owned()),
      headers: headers
        .iter()
        .map(|(name, value)| InboundHeader {
          name: (*name).to_owned(),
          value: (*value).to_owned(),
        })
        .collect(),
      attachments: vec![],
    }
  }

  #[test]
  fn a_raw_message_is_read() {
    let raw = concat!(
      "Authentication-Results: mx.cloudflare.net;\r\n",
      "\tspf=pass smtp.mailfrom=example.com;\r\n",
      "\tdkim=pass header.d=example.com\r\n",
      "From: \"Some One\" <someone@example.com>\r\n",
      "To: help@jsr.io\r\n",
      "Subject: Help\r\n",
      "Message-ID: <a@example.com>\r\n",
      "References: <x@jsr.io>\r\n",
      " <y@jsr.io>\r\n",
      "MIME-Version: 1.0\r\n",
      "Content-Type: multipart/mixed; boundary=\"b\"\r\n",
      "\r\n",
      "--b\r\n",
      "Content-Type: text/plain; charset=utf-8\r\n",
      "\r\n",
      "I cannot publish.\r\n",
      "--b\r\n",
      "Content-Type: text/plain; name=\"log.txt\"\r\n",
      "Content-Disposition: attachment; filename=\"log.txt\"\r\n",
      "Content-Transfer-Encoding: base64\r\n",
      "\r\n",
      "aGVsbG8=\r\n",
      "--b--\r\n",
    );
    let email = InboundEmail::parse(raw.as_bytes()).unwrap();

    assert_eq!(email.from.email, "someone@example.com");
    assert_eq!(email.from.name.as_deref(), Some("Some One"));
    assert_eq!(email.subject.as_deref(), Some("Help"));
    assert_eq!(email.message_id(), Some("<a@example.com>"));
    assert_eq!(
      email.text_body.as_deref().map(str::trim),
      Some("I cannot publish.")
    );

    // Folded headers are read as one value.
    assert_eq!(
      email.referenced_message_ids(),
      vec!["<x@jsr.io>", "<y@jsr.io>"]
    );
    assert!(email.is_verified(Some("mx.cloudflare.net")));

    assert_eq!(email.attachments.len(), 1);
    assert_eq!(email.attachments[0].name, "log.txt");
    assert_eq!(email.attachments[0].content_type, "text/plain");
    assert_eq!(email.attachments[0].content.as_ref(), b"hello");
  }

  #[test]
  fn a_message_without_a_sender_is_not_read() {
    let raw = "Subject: Help\r\nMessage-ID: <a@example.com>\r\n\r\nhi\r\n";
    assert!(InboundEmail::parse(raw.as_bytes()).is_none());
  }

  #[test]
  fn an_oversized_attachment_is_left_out() {
    let raw = format!(
      concat!(
        "From: someone@example.com\r\n",
        "Message-ID: <a@example.com>\r\n",
        "MIME-Version: 1.0\r\n",
        "Content-Type: multipart/mixed; boundary=\"b\"\r\n",
        "\r\n",
        "--b\r\n",
        "Content-Type: text/plain\r\n",
        "\r\n",
        "see attached\r\n",
        "--b\r\n",
        "Content-Type: application/octet-stream\r\n",
        "Content-Disposition: attachment; filename=\"big.bin\"\r\n",
        "\r\n",
        "{}\r\n",
        "--b--\r\n",
      ),
      "x".repeat(MAX_ATTACHMENT_BYTES + 1)
    );
    let email = InboundEmail::parse(raw.as_bytes()).unwrap();
    assert!(email.attachments.is_empty());
    assert_eq!(email.body(), "see attached");
  }

  #[test]
  fn header_lookup_is_case_insensitive() {
    let email = email(&[("message-id", "<a@example.com>")], "hi");
    assert_eq!(email.message_id(), Some("<a@example.com>"));
  }

  #[test]
  fn references_are_split_on_whitespace_and_commas() {
    let email = email(
      &[
        ("In-Reply-To", "<c@jsr.io>"),
        ("References", "<a@jsr.io> <b@jsr.io>,<c@jsr.io>"),
      ],
      "hi",
    );
    // In-Reply-To comes first because it is the most specific, and the id
    // repeated in References is not listed twice.
    assert_eq!(
      email.referenced_message_ids(),
      vec!["<c@jsr.io>", "<a@jsr.io>", "<b@jsr.io>"]
    );
  }

  const TRUSTED: Option<&str> = Some("mx.cloudflare.net");

  #[test]
  fn verification_requires_both_spf_and_dkim() {
    let both = email(
      &[(
        "Authentication-Results",
        "mx.cloudflare.net; spf=pass (sender is permitted) smtp.mailfrom=leo@example.org; dkim=pass header.i=@example.org; dmarc=pass header.from=example.org",
      )],
      "hi",
    );
    assert!(both.is_verified(TRUSTED));

    let spf_only = email(
      &[(
        "Authentication-Results",
        "mx.cloudflare.net; spf=pass; dkim=fail",
      )],
      "hi",
    );
    assert!(!spf_only.is_verified(TRUSTED));

    let neither = email(&[], "hi");
    assert!(!neither.is_verified(TRUSTED));
  }

  #[test]
  fn a_forged_authentication_results_header_cannot_award_a_pass() {
    // A sender can put whatever headers they like in the message they send,
    // including one carrying the trusted server's name. The trusted server
    // prepends its own on receipt, so the real result comes first and the
    // forgery below it must be ignored.
    let forged = email(
      &[
        (
          "Authentication-Results",
          "mx.cloudflare.net; dkim=fail; spf=fail smtp.mailfrom=attacker@example.org",
        ),
        (
          "Authentication-Results",
          "mx.cloudflare.net; dkim=pass; spf=pass smtp.mailfrom=someone@example.org",
        ),
      ],
      "hi",
    );
    assert!(!forged.is_verified(TRUSTED));
  }

  #[test]
  fn results_from_an_untrusted_server_are_ignored() {
    let elsewhere = email(
      &[(
        "Authentication-Results",
        "mx.attacker.example; dkim=pass; spf=pass",
      )],
      "hi",
    );
    assert!(!elsewhere.is_verified(TRUSTED));

    // With no server trusted at all, the header is never consulted.
    let trusted_header = email(
      &[(
        "Authentication-Results",
        "mx.cloudflare.net; dkim=pass; spf=pass",
      )],
      "hi",
    );
    assert!(!trusted_header.is_verified(None));
  }

  #[test]
  fn comments_in_the_header_cannot_fake_a_result() {
    // The parenthesised comment contains both a `;` and the text `spf=pass`,
    // and the real result is a failure.
    let value = "mx.cloudflare.net; spf=fail (sender is not permitted; tried spf=pass) smtp.mailfrom=a@example.org; dkim=fail";
    let stripped = strip_comments(value);
    assert_eq!(method_result(&stripped, "spf").as_deref(), Some("fail"));
    assert_eq!(method_result(&stripped, "dkim").as_deref(), Some("fail"));

    let email = email(&[("Authentication-Results", value)], "hi");
    assert!(!email.is_verified(TRUSTED));
  }

  #[test]
  fn method_names_are_matched_whole() {
    let value = strip_comments("mx.cloudflare.net; dkim-atps=pass; dkim=fail");
    // `dkim-atps` must not be read as `dkim`.
    assert_eq!(method_result(&value, "dkim").as_deref(), Some("fail"));
  }

  #[test]
  fn the_authserv_id_may_carry_a_version() {
    let email = email(
      &[(
        "Authentication-Results",
        "mx.cloudflare.net 1; dkim=pass; spf=pass",
      )],
      "hi",
    );
    assert!(email.is_verified(TRUSTED));
  }

  #[test]
  fn ticket_number_is_found_in_a_reply_subject() {
    let subject = "Re: [TICKET-20260822-04213] Support request";
    assert_eq!(
      TICKET_NUMBER_RE.find(subject).map(|m| m.as_str()),
      Some("TICKET-20260822-04213")
    );
    assert!(TICKET_NUMBER_RE.find("Re: no number here").is_none());
  }

  #[test]
  fn quoted_history_is_cut_from_a_reply() {
    let cases = [
      "new text\n\nOn Mon, 1 Jan 2026 at 10:00, JSR <help@jsr.io> wrote:\n> old text",
      // The attribution wrapped onto a second line.
      "new text\n\nOn Mon, 1 Jan 2026 at 10:00, JSR\n<help@jsr.io> wrote:\n> old text",
      "new text\n\n-----Original Message-----\nFrom: JSR\nold text",
      "new text\n\nFrom: JSR <help@jsr.io>\nSent: Monday, 1 January 2026 10:00\nTo: Someone\n\nold text",
      // No attribution this recognises, only the quote itself.
      "new text\n\nAm 1. Januar 2026 schrieb JSR:\n> old text\n>\n> more old text",
    ];
    for case in cases {
      assert!(
        strip_quoted_history(case).starts_with("new text"),
        "{case:?}"
      );
      assert!(!strip_quoted_history(case).contains("old text"), "{case:?}");
    }
  }

  #[test]
  fn text_that_only_resembles_a_quote_is_kept() {
    // A quoted line with the answer written underneath it is not history.
    let inline = "> did you try again?\nyes, same error";
    assert_eq!(strip_quoted_history(inline), inline);

    let prose = "On Monday it worked.\nFrom: what I can tell it broke later.";
    assert_eq!(strip_quoted_history(prose), prose);
  }

  #[test]
  fn history_is_only_cut_from_replies() {
    let text = "new text\n\nOn Monday, JSR wrote:\n> old text";

    let mut reply = email(&[("In-Reply-To", "<a@jsr.io>")], "hi");
    reply.text_body = Some(text.to_owned());
    assert_eq!(reply.body(), "new text");

    // A first message has no history, so whatever it contains is its own.
    let mut first = email(&[], "hi");
    first.text_body = Some(text.to_owned());
    assert_eq!(first.body(), text);

    // A reply written entirely below the quote would be left empty; all of it
    // is kept instead.
    let below = "On Monday, JSR wrote:\n> old text\n\nnew text";
    reply.text_body = Some(below.to_owned());
    assert_eq!(reply.body(), below);
  }

  #[test]
  fn oversized_bodies_are_truncated_on_a_char_boundary() {
    let mut email = email(&[], "hi");
    // Multi-byte characters so a naive byte cut would split one in half.
    email.text_body = Some("é".repeat(MAX_BODY_BYTES));
    let body = email.body();
    assert!(body.ends_with("[message truncated]"));
    assert!(body.len() < MAX_BODY_BYTES + 64);
  }
}

#[cfg(test)]
mod integration {
  use super::InboundContext;
  use super::InboundError;
  use crate::api::ApiTicket;
  use crate::api::ApiTicketActor;
  use crate::emails::EmailQueue;
  use crate::util::test::ApiResultExt;
  use crate::util::test::TEST_TRUSTED_AUTHSERV_ID;
  use crate::util::test::TestSetup;
  use hyper::StatusCode;
  use serde_json::json;
  use shared::models::TicketStatus;

  /// A raw inbound message from someone@example.com.
  fn inbound(
    message_id: &str,
    subject: &str,
    headers: &[(&str, &str)],
  ) -> String {
    let mut raw = format!(
      "From: Someone <someone@example.com>\r\nTo: help@jsr.io\r\nSubject: {subject}\r\nMessage-ID: {message_id}\r\n"
    );
    for (name, value) in headers {
      raw.push_str(&format!("{name}: {value}\r\n"));
    }
    raw.push_str("\r\nI cannot publish my package.\r\n");
    raw
  }

  async fn try_deliver(t: &TestSetup, raw: &str) -> Result<(), InboundError> {
    super::ingest(
      InboundContext {
        db: &t.db(),
        buckets: &t.buckets(),
        registry_url: &t.registry_url(),
        // No sender is configured under test, so nothing is sent.
        email_sender: None,
        email_queue: &EmailQueue(None),
        trusted_authserv_id: Some(TEST_TRUSTED_AUTHSERV_ID),
      },
      raw.as_bytes(),
    )
    .await
  }

  async fn deliver(t: &TestSetup, raw: String) {
    try_deliver(t, &raw).await.unwrap();
  }

  /// Every ticket in the admin list, which is the only view that shows tickets
  /// nobody has claimed.
  async fn all_tickets(t: &mut TestSetup) -> Vec<ApiTicket> {
    let staff_token = t.staff_user.token.clone();
    let mut resp = t
      .http()
      .get("/api/admin/tickets")
      .token(Some(&staff_token))
      .call()
      .await
      .unwrap();
    let list: crate::api::ApiList<ApiTicket> = resp.expect_ok().await;
    list.items
  }

  /// The claim token of an unclaimed ticket. Not exposed over the API — it only
  /// ever reaches the reporter by email — so it is read from the row.
  async fn claim_token_of(t: &TestSetup, ticket_id: uuid::Uuid) -> uuid::Uuid {
    t.ephemeral_database
      .database
      .as_ref()
      .unwrap()
      .get_ticket(ticket_id)
      .await
      .unwrap()
      .unwrap()
      .0
      .claim_token
      .expect("an unclaimed ticket has a claim token")
  }

  fn message_bodies(overview: &crate::api::ApiTicketOverview) -> Vec<String> {
    overview
      .events
      .iter()
      .filter_map(|event| match event {
        crate::api::ApiTicketMessageOrAuditLog::Message { message } => {
          Some(message.message.clone())
        }
        _ => None,
      })
      .collect()
  }

  /// The stored message rows for a ticket, in order. Used for the parts of a
  /// message that the API deliberately does not expose.
  async fn ticket_messages(
    t: &TestSetup,
    ticket_id: uuid::Uuid,
  ) -> Vec<shared::models::TicketMessage> {
    t.ephemeral_database
      .database
      .as_ref()
      .unwrap()
      .get_ticket(ticket_id)
      .await
      .unwrap()
      .unwrap()
      .2
      .into_iter()
      .map(|(message, ..)| message)
      .collect()
  }

  #[tokio::test]
  async fn inbound_email_opens_a_ticket() {
    let mut t = TestSetup::new().await;

    deliver(&t, inbound("<a@example.com>", "Help", &[])).await;

    let tickets = all_tickets(&mut t).await;
    assert_eq!(tickets.len(), 1);
    let ticket = &tickets[0];

    // Nobody has claimed it, so it is owned by an address rather than a user.
    let ApiTicketActor::Email { email, .. } = &ticket.reporter else {
      panic!("expected an email reporter, got {:?}", ticket.reporter);
    };
    assert_eq!(email, "someone@example.com");
    assert_eq!(ticket.subject.as_deref(), Some("Help"));
    assert_eq!(ticket.status, TicketStatus::Open);
    assert!(ticket.ticket_number.starts_with("TICKET-"));

    // The email itself, plus the acknowledgement sent back to the reporter.
    assert_eq!(ticket.messages.len(), 2);
    assert_eq!(ticket.messages[0].message, "I cannot publish my package.");
    assert!(matches!(ticket.messages[1].author, ApiTicketActor::System));
  }

  #[tokio::test]
  async fn redelivered_email_is_ingested_once() {
    let mut t = TestSetup::new().await;

    let payload = inbound("<dup@example.com>", "Help", &[]);
    deliver(&t, payload.clone()).await;
    // A sending server retries until the message is accepted, so a repeat must
    // not fail either.
    deliver(&t, payload).await;

    assert_eq!(all_tickets(&mut t).await.len(), 1);
  }

  #[tokio::test]
  async fn reply_threads_onto_the_ticket_it_answers() {
    let mut t = TestSetup::new().await;

    deliver(&t, inbound("<first@example.com>", "Help", &[])).await;
    let ticket = all_tickets(&mut t).await.remove(0);
    // The Message-ID the acknowledgement went out under is not part of the API
    // surface, so it is read from the row that records it.
    let auto_reply_id = ticket_messages(&t, ticket.id)
      .await
      .pop()
      .unwrap()
      .email_message_id
      .expect("the auto-reply is sent with a Message-ID");

    deliver(
      &t,
      inbound(
        "<second@example.com>",
        "Re: Help",
        &[("In-Reply-To", &auto_reply_id)],
      ),
    )
    .await;

    let tickets = all_tickets(&mut t).await;
    assert_eq!(tickets.len(), 1, "the reply must not open a second ticket");
    assert_eq!(tickets[0].messages.len(), 3);
    assert_eq!(tickets[0].status, TicketStatus::WaitingOnSupport);
  }

  #[tokio::test]
  async fn reply_threads_by_ticket_number_when_headers_are_lost() {
    let mut t = TestSetup::new().await;

    deliver(&t, inbound("<first@example.com>", "Help", &[])).await;
    let ticket = all_tickets(&mut t).await.remove(0);

    // No In-Reply-To or References at all: the ticket number in the subject is
    // the only thing tying this message to the conversation.
    deliver(
      &t,
      inbound(
        "<second@example.com>",
        &format!("Re: [{}] Support request", ticket.ticket_number),
        &[],
      ),
    )
    .await;

    let tickets = all_tickets(&mut t).await;
    assert_eq!(tickets.len(), 1);
    assert_eq!(tickets[0].messages.len(), 3);
  }

  #[tokio::test]
  async fn reply_reopens_a_closed_ticket() {
    let mut t = TestSetup::new().await;

    deliver(&t, inbound("<first@example.com>", "Help", &[])).await;
    let ticket = all_tickets(&mut t).await.remove(0);

    let staff_token = t.staff_user.token.clone();
    let mut resp = t
      .http()
      .patch(format!("/api/admin/tickets/{}", ticket.id))
      .token(Some(&staff_token))
      .body_json(json!({ "status": "closed" }))
      .call()
      .await
      .unwrap();
    let closed: ApiTicket = resp.expect_ok().await;
    assert_eq!(closed.status, TicketStatus::Closed);

    deliver(
      &t,
      inbound(
        "<second@example.com>",
        &format!("Re: [{}] Support request", ticket.ticket_number),
        &[],
      ),
    )
    .await;

    let tickets = all_tickets(&mut t).await;
    assert_eq!(tickets[0].status, TicketStatus::WaitingOnSupport);
  }

  #[tokio::test]
  async fn attachments_are_stored_and_served_back() {
    let mut t = TestSetup::new().await;

    let content = b"hello from an attachment";
    let raw = concat!(
      "From: Someone <someone@example.com>\r\n",
      "Subject: Help\r\n",
      "Message-ID: <a@example.com>\r\n",
      "MIME-Version: 1.0\r\n",
      "Content-Type: multipart/mixed; boundary=\"b\"\r\n",
      "\r\n",
      "--b\r\n",
      "Content-Type: text/plain\r\n",
      "\r\n",
      "I cannot publish my package.\r\n",
      "--b\r\n",
      "Content-Type: text/plain; name=\"log.txt\"\r\n",
      "Content-Disposition: attachment; filename=\"log.txt\"\r\n",
      "\r\n",
      "hello from an attachment\r\n",
      "--b--\r\n",
    );
    deliver(&t, raw.to_owned()).await;

    let ticket = all_tickets(&mut t).await.remove(0);
    let attachment = &ticket.messages[0].attachments[0];
    assert_eq!(attachment.filename, "log.txt");
    assert_eq!(attachment.size_bytes, content.len() as i32);

    let staff_token = t.staff_user.token.clone();
    let mut resp = t
      .http()
      .get(format!(
        "/api/tickets/{}/attachments/{}",
        ticket.id, attachment.id
      ))
      .token(Some(&staff_token))
      .call()
      .await
      .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);

    // Served as opaque bytes, never under the sender's claimed content type:
    // an HTML or SVG "attachment" must not be able to run script on the API
    // origin.
    assert_eq!(
      resp.headers().get(hyper::header::CONTENT_TYPE).unwrap(),
      "application/octet-stream"
    );
    let disposition = resp
      .headers()
      .get(hyper::header::CONTENT_DISPOSITION)
      .unwrap()
      .to_str()
      .unwrap();
    assert!(disposition.starts_with("attachment;"), "{disposition}");
    assert!(disposition.contains("log.txt"), "{disposition}");

    let body =
      axum::body::to_bytes(std::mem::take(resp.body_mut()), usize::MAX)
        .await
        .unwrap();
    assert_eq!(body.as_ref(), content);

    // A stranger cannot pull the file out of somebody else's ticket.
    let user2_token = t.user2.token.clone();
    let mut resp = t
      .http()
      .get(format!(
        "/api/tickets/{}/attachments/{}",
        ticket.id, attachment.id
      ))
      .token(Some(&user2_token))
      .call()
      .await
      .unwrap();
    resp.expect_err(StatusCode::NOT_FOUND).await;
  }

  #[tokio::test]
  async fn inbound_email_queues_an_auto_reply() {
    let mut t = TestSetup::new().await;

    deliver(&t, inbound("<a@example.com>", "Help", &[])).await;
    let ticket = all_tickets(&mut t).await.remove(0);

    // No sender is configured under test, so nothing is queued and the ticket
    // still lands. What matters is that the delivery succeeded and the
    // auto-reply is recorded on the ticket either way.
    let db = t.ephemeral_database.database.as_ref().unwrap();
    assert!(
      db.list_stale_email_deliveries(0, 10)
        .await
        .unwrap()
        .is_empty(),
      "no deliveries are queued when email is not configured"
    );
    assert_eq!(ticket.messages.len(), 2);
  }

  #[tokio::test]
  async fn a_queued_delivery_is_sent_once_and_then_ignored() {
    let t = TestSetup::new().await;
    let db = t.ephemeral_database.database.as_ref().unwrap();

    let id = db
      .enqueue_email(shared::models::NewEmailDelivery {
        to_address: "someone@example.com".to_owned(),
        subject: "[TICKET-20260824-00001] Support request".to_owned(),
        body_text: "hello".to_owned(),
        body_html: "<p>hello</p>".to_owned(),
        message_id: Some("<x@jsr.io>".to_owned()),
        in_reply_to: None,
        reference_ids: vec![],
      })
      .await
      .unwrap();

    // Queued but not yet delivered, so the sweeper can still see it.
    let pending = db.get_pending_email_delivery(id).await.unwrap();
    assert!(pending.is_some());
    assert_eq!(
      db.list_stale_email_deliveries(0, 10).await.unwrap(),
      vec![id]
    );

    db.mark_email_delivery_sent(id, "<sent@example.net>")
      .await
      .unwrap();

    // Once sent it is invisible to both the delivery path and the sweeper, so a
    // task the queue redelivers cannot produce a second email.
    assert!(db.get_pending_email_delivery(id).await.unwrap().is_none());
    assert!(
      db.list_stale_email_deliveries(0, 10)
        .await
        .unwrap()
        .is_empty()
    );
  }

  #[tokio::test]
  async fn the_sent_message_id_replaces_the_queued_one() {
    let mut t = TestSetup::new().await;
    deliver(&t, inbound("<a@example.com>", "Help", &[])).await;
    let ticket = all_tickets(&mut t).await.remove(0);
    let db = t.db();

    // The auto-reply is on the ticket under the Message-ID it was queued with.
    let queued_id = ticket_messages(&t, ticket.id)
      .await
      .pop()
      .unwrap()
      .email_message_id
      .unwrap();

    let delivery = |message_id: &str, references: Vec<String>| {
      shared::models::NewEmailDelivery {
        to_address: "someone@example.com".to_owned(),
        subject: "subject".to_owned(),
        body_text: "text".to_owned(),
        body_html: "<p>html</p>".to_owned(),
        message_id: Some(message_id.to_owned()),
        in_reply_to: references.last().cloned(),
        reference_ids: references,
      }
    };
    let first = db
      .enqueue_email(delivery(&queued_id, vec![]))
      .await
      .unwrap();
    // A later email in the thread, queued before the first one went out.
    let second = db
      .enqueue_email(delivery(
        "<later@jsr.io>",
        vec!["<a@example.com>".to_owned(), queued_id.clone()],
      ))
      .await
      .unwrap();

    db.mark_email_delivery_sent(first, "<sent@example.net>")
      .await
      .unwrap();

    // A reply to the email names the ID it was actually sent under.
    let on_ticket = ticket_messages(&t, ticket.id).await.pop().unwrap();
    assert_eq!(
      on_ticket.email_message_id.as_deref(),
      Some("<sent@example.net>")
    );
    deliver(
      &t,
      inbound(
        "<b@example.com>",
        "Re: Help",
        &[("In-Reply-To", "<sent@example.net>")],
      ),
    )
    .await;
    assert_eq!(all_tickets(&mut t).await.len(), 1);

    // And the email still waiting refers to it too.
    let later = db
      .get_pending_email_delivery(second)
      .await
      .unwrap()
      .unwrap();
    assert_eq!(later.in_reply_to.as_deref(), Some("<sent@example.net>"));
    assert_eq!(
      later.reference_ids,
      vec!["<a@example.com>", "<sent@example.net>"]
    );
  }

  #[tokio::test]
  async fn a_delivery_is_not_lost_when_no_sender_is_configured() {
    let t = TestSetup::new().await;
    let db = t.ephemeral_database.database.as_ref().unwrap();

    let id = db
      .enqueue_email(shared::models::NewEmailDelivery {
        to_address: "someone@example.com".to_owned(),
        subject: "subject".to_owned(),
        body_text: "text".to_owned(),
        body_html: "<p>html</p>".to_owned(),
        message_id: None,
        in_reply_to: None,
        reference_ids: vec![],
      })
      .await
      .unwrap();

    // The queue dispatches to a service that has no sender. This
    // used to be swallowed with a 200, which marked the delivery done without
    // ever sending it. It must be recorded as a failure instead, so the mail is
    // still there to send once the configuration is fixed.
    let terminal = crate::emails::deliver(db, None, id).await.unwrap();
    assert!(!terminal, "a missing sender must not end the delivery");

    let pending = db.get_pending_email_delivery(id).await.unwrap().unwrap();
    assert_eq!(pending.attempts, 1);
    assert!(pending.last_error.unwrap().contains("no email sender"));
    assert!(pending.sent_at.is_none());

    // And it is still visible to the sweeper, so it goes out after a fix.
    assert_eq!(
      db.list_stale_email_deliveries(0, 10).await.unwrap(),
      vec![id]
    );
  }

  #[tokio::test]
  async fn a_delivery_is_abandoned_after_too_many_failures() {
    let t = TestSetup::new().await;
    let db = t.ephemeral_database.database.as_ref().unwrap();

    let id = db
      .enqueue_email(shared::models::NewEmailDelivery {
        to_address: "nobody@example.invalid".to_owned(),
        subject: "subject".to_owned(),
        body_text: "text".to_owned(),
        body_html: "<p>html</p>".to_owned(),
        message_id: None,
        in_reply_to: None,
        reference_ids: vec![],
      })
      .await
      .unwrap();

    // Below the limit it stays retryable.
    let abandoned = db
      .record_email_delivery_failure(id, "boom", 3)
      .await
      .unwrap();
    assert!(!abandoned);
    assert!(db.get_pending_email_delivery(id).await.unwrap().is_some());

    let abandoned = db
      .record_email_delivery_failure(id, "boom", 3)
      .await
      .unwrap();
    assert!(!abandoned);

    // The third failure hits the limit and takes it out of the queue for good,
    // rather than retrying an undeliverable address forever.
    let abandoned = db
      .record_email_delivery_failure(id, "boom", 3)
      .await
      .unwrap();
    assert!(abandoned);
    assert!(db.get_pending_email_delivery(id).await.unwrap().is_none());
    assert!(
      db.list_stale_email_deliveries(0, 10)
        .await
        .unwrap()
        .is_empty(),
      "an abandoned delivery is not swept up again"
    );
  }

  #[tokio::test]
  async fn staff_can_reply_to_a_ticket_nobody_has_claimed() {
    let mut t = TestSetup::new().await;

    deliver(&t, inbound("<a@example.com>", "Help", &[])).await;
    let ticket = all_tickets(&mut t).await.remove(0);
    assert_eq!(ticket.status, TicketStatus::Open);

    // The ticket has no creator until it is claimed, so anything comparing the
    // author against it has to cope with a NULL on the other side. This is the
    // ordinary path for answering support mail, and it used to 500.
    let staff_token = t.staff_user.token.clone();
    let mut resp = t
      .http()
      .post(format!("/api/tickets/{}", ticket.id))
      .token(Some(&staff_token))
      .body_json(json!({ "message": "Have you tried publishing again?" }))
      .call()
      .await
      .unwrap();
    let reply: crate::api::ApiTicketMessage = resp.expect_ok().await;
    assert_eq!(reply.message, "Have you tried publishing again?");
    assert_eq!(
      reply.direction,
      shared::models::TicketMessageDirection::Outbound
    );

    // Staff spoke last, so the reporter is the one now owed a response.
    let after = all_tickets(&mut t).await.remove(0);
    assert_eq!(after.status, TicketStatus::WaitingOnUser);
  }

  #[tokio::test]
  async fn a_staff_note_is_never_shown_to_the_reporter() {
    let mut t = TestSetup::new().await;

    deliver(&t, inbound("<a@example.com>", "Help", &[])).await;
    let ticket = all_tickets(&mut t).await.remove(0);

    let staff_token = t.staff_user.token.clone();
    let mut resp = t
      .http()
      .post(format!("/api/tickets/{}", ticket.id))
      .token(Some(&staff_token))
      .body_json(
        json!({ "message": "looks like a squatter", "internal": true }),
      )
      .call()
      .await
      .unwrap();
    let note: crate::api::ApiTicketMessage = resp.expect_ok().await;
    assert!(note.internal);

    // Staff see it.
    let mut resp = t
      .http()
      .get(format!("/api/tickets/{}", ticket.id))
      .token(Some(&staff_token))
      .call()
      .await
      .unwrap();
    let staff_view: crate::api::ApiTicketOverview = resp.expect_ok().await;
    assert!(
      message_bodies(&staff_view)
        .iter()
        .any(|m| m == "looks like a squatter")
    );

    // The reporter, holding the claim token from their auto-reply, does not.
    let claim_token = claim_token_of(&t, ticket.id).await;
    let mut resp = t
      .http()
      .get(format!("/api/tickets/{}?claim={claim_token}", ticket.id))
      .token(None)
      .call()
      .await
      .unwrap();
    let reporter_view: crate::api::ApiTicketOverview = resp.expect_ok().await;
    let bodies = message_bodies(&reporter_view);
    assert!(
      !bodies.iter().any(|m| m == "looks like a squatter"),
      "the note leaked to the reporter: {bodies:?}"
    );
    // The rest of the conversation is still there.
    assert!(bodies.iter().any(|m| m == "I cannot publish my package."));
  }

  #[tokio::test]
  async fn a_staff_note_does_not_survive_claiming_the_ticket() {
    let mut t = TestSetup::new().await;

    deliver(&t, inbound("<a@example.com>", "Help", &[])).await;
    let ticket = all_tickets(&mut t).await.remove(0);
    let claim_token = claim_token_of(&t, ticket.id).await;

    let staff_token = t.staff_user.token.clone();
    t.http()
      .post(format!("/api/tickets/{}", ticket.id))
      .token(Some(&staff_token))
      .body_json(json!({ "message": "internal only", "internal": true }))
      .call()
      .await
      .unwrap();

    // Claiming returns the ticket to the person who just claimed it, who is not
    // staff however the ticket got to them.
    let user_token = t.user1.token.clone();
    let mut resp = t
      .http()
      .post(format!(
        "/api/tickets/{}/claim?claim={claim_token}",
        ticket.id
      ))
      .token(Some(&user_token))
      .call()
      .await
      .unwrap();
    let claimed: ApiTicket = resp.expect_ok().await;
    assert!(
      !claimed
        .messages
        .iter()
        .any(|m| m.message == "internal only"),
      "the note leaked in the claim response"
    );

    // Nor in their own account view of it afterwards.
    let mut resp = t
      .http()
      .get("/api/user/tickets")
      .token(Some(&user_token))
      .call()
      .await
      .unwrap();
    let own: Vec<ApiTicket> = resp.expect_ok().await;
    assert!(
      !own
        .iter()
        .flat_map(|t| &t.messages)
        .any(|m| m.message == "internal only"),
      "the note leaked in the account ticket list"
    );
  }

  #[tokio::test]
  async fn a_staff_member_can_note_on_a_ticket_they_opened_themselves() {
    let mut t = TestSetup::new().await;

    // Staff open tickets like anyone else — filing one about their own scope,
    // or just trying something out. The author then *is* the creator, which is
    // the one combination where a note is written by the person the ticket
    // belongs to.
    let staff_token = t.staff_user.token.clone();
    let mut resp = t
      .http()
      .post("/api/tickets")
      .token(Some(&staff_token))
      .body_json(json!({
        "kind": shared::models::TicketKind::Other,
        "meta": {},
        "message": "my own ticket",
      }))
      .call()
      .await
      .unwrap();
    let own: ApiTicket = resp.expect_ok().await;

    let mut resp = t
      .http()
      .post(format!("/api/tickets/{}", own.id))
      .token(Some(&staff_token))
      .body_json(
        json!({ "message": "note on my own ticket", "internal": true }),
      )
      .call()
      .await
      .unwrap();
    let note: crate::api::ApiTicketMessage = resp.expect_ok().await;
    assert!(note.internal);
    // A note is from the JSR side whoever writes it, so it is outbound even
    // here. Anything else violates the check constraint on the table.
    assert_eq!(
      note.direction,
      shared::models::TicketMessageDirection::Outbound
    );
  }

  #[tokio::test]
  async fn a_note_cannot_be_written_by_anyone_but_staff() {
    let mut t = TestSetup::new().await;

    deliver(&t, inbound("<a@example.com>", "Help", &[])).await;
    let ticket = all_tickets(&mut t).await.remove(0);
    let claim_token = claim_token_of(&t, ticket.id).await;

    // The reporter, via their claim link.
    let mut resp = t
      .http()
      .post(format!("/api/tickets/{}?claim={claim_token}", ticket.id))
      .token(None)
      .body_json(json!({ "message": "sneaky", "internal": true }))
      .call()
      .await
      .unwrap();
    resp.expect_err(StatusCode::FORBIDDEN).await;

    // And an ordinary signed-in account that owns a ticket of its own.
    let user_token = t.user1.token.clone();
    let mut resp = t
      .http()
      .post("/api/tickets")
      .token(Some(&user_token))
      .body_json(json!({
        "kind": shared::models::TicketKind::Other,
        "meta": {},
        "message": "hello",
      }))
      .call()
      .await
      .unwrap();
    let own: ApiTicket = resp.expect_ok().await;

    let mut resp = t
      .http()
      .post(format!("/api/tickets/{}", own.id))
      .token(Some(&user_token))
      .body_json(json!({ "message": "sneaky", "internal": true }))
      .call()
      .await
      .unwrap();
    resp.expect_err(StatusCode::FORBIDDEN).await;
  }

  #[tokio::test]
  async fn a_staff_note_sends_no_email_and_does_not_move_the_status() {
    let mut t = TestSetup::new().await;

    deliver(&t, inbound("<a@example.com>", "Help", &[])).await;
    let ticket = all_tickets(&mut t).await.remove(0);
    assert_eq!(ticket.status, TicketStatus::Open);

    let staff_token = t.staff_user.token.clone();
    t.http()
      .post(format!("/api/tickets/{}", ticket.id))
      .token(Some(&staff_token))
      .body_json(json!({ "message": "note to self", "internal": true }))
      .call()
      .await
      .unwrap();

    // A reply would have moved this to waiting_on_user. A note says nothing to
    // the reporter, so it must not.
    let after = all_tickets(&mut t).await.remove(0);
    assert_eq!(after.status, TicketStatus::Open);

    // And nothing was queued to be sent.
    let db = t.ephemeral_database.database.as_ref().unwrap();
    assert!(
      db.list_stale_email_deliveries(0, 10)
        .await
        .unwrap()
        .is_empty()
    );
  }

  #[tokio::test]
  async fn email_without_a_message_id_is_refused() {
    let mut t = TestSetup::new().await;

    let raw =
      "From: Someone <someone@example.com>\r\nSubject: Help\r\n\r\nhi\r\n";
    // Refused for good rather than failed: delivering it again would not help.
    assert!(matches!(
      try_deliver(&t, raw).await,
      Err(InboundError::Rejected(_))
    ));

    assert!(all_tickets(&mut t).await.is_empty());
  }

  #[tokio::test]
  async fn sender_verification_is_recorded_on_the_message() {
    let mut t = TestSetup::new().await;

    let results =
      format!("{TEST_TRUSTED_AUTHSERV_ID}; spf=pass; dkim=pass; dmarc=pass");
    // The receiving server's header sits above everything the sender wrote.
    let verified = format!(
      "Authentication-Results: {results}\r\n{}",
      inbound("<a@example.com>", "Help", &[])
    );
    deliver(&t, verified).await;
    deliver(&t, inbound("<b@example.com>", "Other", &[])).await;

    let mut tickets = all_tickets(&mut t).await;
    tickets.sort_by(|a, b| a.subject.cmp(&b.subject));
    let first = ticket_messages(&t, tickets[0].id).await.remove(0);
    assert_eq!(first.author_email_verified, Some(true));
    let second = ticket_messages(&t, tickets[1].id).await.remove(0);
    assert_eq!(second.author_email_verified, Some(false));
  }

  #[tokio::test]
  async fn claim_token_grants_access_and_binds_the_ticket() {
    let mut t = TestSetup::new().await;

    deliver(&t, inbound("<a@example.com>", "Help", &[])).await;
    let ticket = all_tickets(&mut t).await.remove(0);

    // The claim token is not handed out over the API — it only reaches the
    // reporter by email — so it is read straight from the database here.
    let claim_token = t
      .ephemeral_database
      .database
      .as_ref()
      .unwrap()
      .get_ticket(ticket.id)
      .await
      .unwrap()
      .unwrap()
      .0
      .claim_token
      .expect("an unclaimed ticket has a claim token");

    let user2_token = t.user2.token.clone();

    // A signed-in stranger without the token cannot see it at all.
    let mut resp = t
      .http()
      .get(format!("/api/tickets/{}", ticket.id))
      .token(Some(&user2_token))
      .call()
      .await
      .unwrap();
    resp.expect_err(StatusCode::NOT_FOUND).await;

    // With the token, they can read the thread.
    let mut resp = t
      .http()
      .get(format!("/api/tickets/{}?claim={claim_token}", ticket.id))
      .token(None)
      .call()
      .await
      .unwrap();
    let overview: crate::api::ApiTicketOverview = resp.expect_ok().await;
    assert_eq!(overview.id, ticket.id);

    // A wrong token is worth no more than none at all.
    let mut resp = t
      .http()
      .get(format!(
        "/api/tickets/{}?claim={}",
        ticket.id,
        uuid::Uuid::new_v4()
      ))
      .token(None)
      .call()
      .await
      .unwrap();
    resp.expect_err(StatusCode::UNAUTHORIZED).await;

    // Claiming binds the ticket to the account and spends the token.
    let user1_token = t.user1.token.clone();
    let mut resp = t
      .http()
      .post(format!(
        "/api/tickets/{}/claim?claim={claim_token}",
        ticket.id
      ))
      .token(Some(&user1_token))
      .call()
      .await
      .unwrap();
    let claimed: ApiTicket = resp.expect_ok().await;
    let ApiTicketActor::User { user: owner } = &claimed.reporter else {
      panic!("expected a user reporter after claiming");
    };
    assert_eq!(owner.id, t.user1.user.id);

    // The same link cannot be replayed by somebody else.
    let mut resp = t
      .http()
      .post(format!(
        "/api/tickets/{}/claim?claim={claim_token}",
        ticket.id
      ))
      .token(Some(&user2_token))
      .call()
      .await
      .unwrap();
    resp.expect_err(StatusCode::BAD_REQUEST).await;
  }
}
