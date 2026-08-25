// Copyright 2024 the JSR authors. All rights reserved. MIT license.

//! Inbound email. Postmark POSTs a webhook here for every message sent to the
//! support address; each one either opens a support ticket or is appended to the
//! ticket it is replying to.

use base64::Engine;
use bytes::Bytes;
use hyper::Body;
use hyper::Request;
use hyper::Response;
use hyper::StatusCode;
use routerify::Router;
use routerify::prelude::RequestExt;
use serde::Deserialize;
use std::borrow::Cow;
use std::sync::LazyLock;
use tracing::Span;
use tracing::field;
use tracing::instrument;
use uuid::Uuid;

use crate::RegistryUrl;
use crate::db::Database;
use crate::db::NewEmailTicket;
use crate::db::NewTicketAttachment;
use crate::db::NewTicketEmailMessage;
use crate::db::NewTicketSystemMessage;
use crate::emails;
use crate::emails::EmailArgs;
use crate::emails::EmailQueue;
use crate::emails::EmailSender;
use crate::emails::EmailThread;
use crate::s3::Buckets;
use crate::s3::S3UploadOptions;
use crate::s3::UploadTaskBody;
use crate::util;
use crate::util::ApiResult;

use super::ApiError;

/// The password Postmark authenticates with. `None` disables inbound handling.
pub struct PostmarkWebhookPassword(pub Option<String>);

/// The `authserv-id` of the mail server that receives support mail before it
/// reaches Postmark — `mx.google.com` for a Google Workspace inbox.
///
/// Mail forwarded from that inbox arrives at Postmark from the forwarder's own
/// servers, so the SPF check Postmark performs is against the wrong sender and
/// fails for almost everything. The forwarder recorded the real result when it
/// first received the message, and that record survives the hop, so it is what
/// we read instead — but only for the server named here, since anyone can write
/// an `Authentication-Results` header claiming whatever they like.
///
/// `None` means no upstream is trusted and only Postmark's own check is used.
pub struct InboundTrustedAuthservId(pub Option<String>);

/// Files above this are dropped rather than stored. Postmark caps an inbound
/// message at 35 MB in total; this is well under that, and comfortably above
/// anything a support conversation legitimately needs.
const MAX_ATTACHMENT_BYTES: usize = 10 * 1024 * 1024;

/// Guards against one message filling the bucket with many small files.
const MAX_ATTACHMENTS_PER_MESSAGE: usize = 20;

/// Stored message bodies are capped so that a runaway email cannot bloat a row
/// without bound. Anything longer is truncated with a marker.
const MAX_BODY_BYTES: usize = 256 * 1024;

pub fn hooks_router() -> Router<Body, ApiError> {
  Router::builder()
    .post("/postmark", postmark_inbound_handler)
    .build()
    .unwrap()
}

/// The subset of Postmark's inbound webhook payload we act on.
///
/// Deserialised loosely on purpose: Postmark adds fields over time, and an
/// unknown one must not turn a real support email into a rejected delivery.
#[derive(Debug, Deserialize)]
#[serde(rename_all = "PascalCase")]
struct InboundEmail {
  from_full: InboundAddress,
  subject: Option<String>,
  text_body: Option<String>,
  /// The message with the quoted history below it removed. Present on replies
  /// only, and preferred over `text_body` so a long thread is not re-stored in
  /// full on every round trip.
  stripped_text_reply: Option<String>,
  #[serde(default)]
  headers: Vec<InboundHeader>,
  #[serde(default)]
  attachments: Vec<InboundAttachment>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "PascalCase")]
struct InboundAddress {
  email: String,
  name: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "PascalCase")]
struct InboundHeader {
  name: String,
  value: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "PascalCase")]
struct InboundAttachment {
  name: String,
  /// Base64, as Postmark delivers it.
  content: String,
  content_type: String,
}

impl InboundEmail {
  fn header(&self, name: &str) -> Option<&str> {
    self
      .headers
      .iter()
      .find(|header| header.name.eq_ignore_ascii_case(name))
      .map(|header| header.value.as_str())
  }

  /// The sending domain's own `Message-ID`. Without one there is nothing to
  /// deduplicate on, so such a message is refused rather than risk being
  /// ingested twice.
  fn message_id(&self) -> Option<&str> {
    self.header("Message-ID")
  }

  /// Whether the sending domain passed both SPF and DKIM.
  ///
  /// Postmark's own check is only meaningful for mail delivered to it directly.
  /// When support mail is forwarded from another inbox, Postmark sees the
  /// forwarder as the sender and SPF fails no matter who actually wrote in, so
  /// the trusted upstream's own record is preferred where there is one.
  ///
  /// Unverified mail still opens a ticket — a misconfigured sender with a real
  /// problem should not be silently dropped — but the flag is recorded and
  /// surfaced so nobody treats the address as proven.
  fn is_verified(&self, trusted_authserv_id: Option<&str>) -> bool {
    if let Some(trusted) = trusted_authserv_id
      && let Some(results) = self.trusted_auth_results(trusted)
    {
      return method_result(&results, "spf").as_deref() == Some("pass")
        && method_result(&results, "dkim").as_deref() == Some("pass");
    }

    let spam_test = self.header("X-Spam-Test").unwrap_or_default();
    spam_test.contains("SPF_PASS") && spam_test.contains("DKIM_VALID")
  }

  /// The `Authentication-Results` written by the trusted upstream, if it is
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

  fn body(&self) -> String {
    let body = self
      .stripped_text_reply
      .as_deref()
      .filter(|body| !body.trim().is_empty())
      .or(self.text_body.as_deref())
      .unwrap_or_default()
      .trim();

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

/// Compares two secrets without leaking which byte differed through timing.
fn secret_eq(a: &str, b: &str) -> bool {
  if a.len() != b.len() {
    return false;
  }
  a.bytes()
    .zip(b.bytes())
    .fold(0u8, |acc, (x, y)| acc | (x ^ y))
    == 0
}

#[instrument(name = "POST /api/hooks/postmark", skip(req), fields(ticket_id))]
pub async fn postmark_inbound_handler(
  mut req: Request<Body>,
) -> ApiResult<Response<Body>> {
  let password = req.data::<PostmarkWebhookPassword>().unwrap();
  let Some(password) = &password.0 else {
    return Err(ApiError::TicketNotFound);
  };

  let expected = format!(
    "Basic {}",
    base64::engine::general_purpose::STANDARD
      .encode(format!("webhook:{password}"))
  );
  let provided = req
    .headers()
    .get(hyper::header::AUTHORIZATION)
    .and_then(|value| value.to_str().ok())
    .unwrap_or_default();
  if !secret_eq(provided, &expected) {
    return Err(ApiError::MissingAuthentication);
  }

  let email: InboundEmail = util::decode_json(&mut req).await?;

  let Some(email_message_id) = email.message_id().map(str::to_owned) else {
    // Nothing to deduplicate on, so ingesting this could duplicate the message
    // on every Postmark retry. A 400 is honest: the message is malformed, and
    // Postmark will not keep retrying it.
    tracing::warn!("rejecting inbound email with no Message-ID header");
    return Err(ApiError::MalformedRequest {
      msg: Cow::Borrowed("inbound email is missing its Message-ID header"),
    });
  };

  let db = req.data::<Database>().unwrap();

  let ticket_id = resolve_ticket(db, &email).await?;
  if let Some(ticket_id) = ticket_id {
    Span::current().record("ticket_id", field::display(ticket_id));
  }

  // Uploads happen before the row is written so the storage key can name the
  // message it belongs to. A failure later leaves unreferenced objects behind,
  // which is preferable to rows pointing at files that were never stored.
  let message_id = Uuid::new_v4();
  let buckets = req.data::<Buckets>().unwrap();
  let attachments =
    store_attachments(buckets, message_id, &email.attachments).await?;

  let message = NewTicketEmailMessage {
    id: message_id,
    author_email: email.from_full.email.clone(),
    author_name: email.from_full.name.clone().filter(|n| !n.is_empty()),
    author_email_verified: email.is_verified(
      req.data::<InboundTrustedAuthservId>().unwrap().0.as_deref(),
    ),
    email_message_id,
    message: email.body(),
    attachments,
  };

  match ticket_id {
    Some(ticket_id) => {
      if db
        .ticket_add_email_message(ticket_id, message)
        .await?
        .is_none()
      {
        tracing::info!("ignoring already-ingested inbound email");
      }
    }
    None => open_ticket(&req, message, &email).await?,
  }

  Ok(util::create_response(StatusCode::OK, "text/plain", "OK"))
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
  req: &Request<Body>,
  message: NewTicketEmailMessage,
  email: &InboundEmail,
) -> Result<(), ApiError> {
  let db = req.data::<Database>().unwrap();
  let registry_url = req.data::<RegistryUrl>().unwrap();
  let email_sender = req.data::<Option<EmailSender>>().unwrap();

  let subject = email
    .subject
    .clone()
    .filter(|subject| !subject.trim().is_empty())
    .unwrap_or_else(|| "(no subject)".to_owned());

  let claim_token = Uuid::new_v4();
  let domain = registry_url.0.host_str().unwrap_or("jsr.io");
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

  let Some((ticket, ..)) = db.create_ticket_from_email(new_ticket).await?
  else {
    tracing::info!("ignoring already-ingested inbound email");
    return Ok(());
  };

  Span::current().record("ticket_id", field::display(ticket.id));

  let Some(email_sender) = email_sender else {
    return Ok(());
  };

  let claim_url = format!(
    "{}ticket/{}?claim={}",
    registry_url.0, ticket.id, claim_token
  );
  let email_args = EmailArgs::SupportTicketAutoReply {
    name: reporter_name.map(Cow::Owned),
    ticket_number: Cow::Borrowed(&ticket.ticket_number),
    original_subject: Cow::Borrowed(&subject),
    claim_url: Cow::Borrowed(&claim_url),
    registry_url: Cow::Borrowed(registry_url.0.as_str()),
    registry_name: Cow::Borrowed(&email_sender.from_name),
    support_email: Cow::Borrowed(&email_sender.from),
  };

  // Queued rather than sent here: the ticket is already on file, and a Postmark
  // failure must not turn into a non-2xx. Postmark would redeliver the webhook,
  // find the message already ingested, and skip past the send entirely — so the
  // reporter would never be acknowledged. The delivery row is retried instead.
  if let Err(err) = emails::enqueue(
    db,
    email_sender,
    req.data::<EmailQueue>().unwrap(),
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
///
/// Files that are too large, too numerous, or not decodable are skipped with a
/// warning rather than failing the delivery — losing an attachment is better
/// than losing the support request it came with.
async fn store_attachments(
  buckets: &Buckets,
  message_id: Uuid,
  attachments: &[InboundAttachment],
) -> Result<Vec<NewTicketAttachment>, ApiError> {
  let mut out = Vec::new();

  for attachment in attachments.iter().take(MAX_ATTACHMENTS_PER_MESSAGE) {
    let Ok(content) =
      base64::engine::general_purpose::STANDARD.decode(&attachment.content)
    else {
      tracing::warn!(
        filename = %attachment.name,
        "skipping inbound attachment that is not valid base64"
      );
      continue;
    };

    if content.len() > MAX_ATTACHMENT_BYTES {
      tracing::warn!(
        filename = %attachment.name,
        size = content.len(),
        "skipping oversized inbound attachment"
      );
      continue;
    }

    let attachment_id = Uuid::new_v4();
    let storage_key = format!("tickets/{message_id}/{attachment_id}");
    let size_bytes = content.len();

    buckets
      .ticket_attachments_bucket
      .upload(
        storage_key.clone().into(),
        UploadTaskBody::Bytes(Bytes::from(content)),
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
      filename: attachment.name.clone(),
      content_type: attachment.content_type.clone(),
      size_bytes: size_bytes as i32,
      storage_key,
    });
  }

  if attachments.len() > MAX_ATTACHMENTS_PER_MESSAGE {
    tracing::warn!(
      total = attachments.len(),
      kept = MAX_ATTACHMENTS_PER_MESSAGE,
      "inbound email had more attachments than we store"
    );
  }

  Ok(out)
}

#[cfg(test)]
mod test {
  use super::*;

  fn email(headers: &[(&str, &str)], subject: &str) -> InboundEmail {
    InboundEmail {
      from_full: InboundAddress {
        email: "someone@example.com".to_owned(),
        name: Some("Someone".to_owned()),
      },
      subject: Some(subject.to_owned()),
      text_body: Some("body".to_owned()),
      stripped_text_reply: None,
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

  const TRUSTED: Option<&str> = Some("mx.google.com");

  #[test]
  fn verification_requires_both_spf_and_dkim() {
    let both = email(&[("X-Spam-Test", "SPF_PASS,DKIM_VALID")], "hi");
    assert!(both.is_verified(None));

    let spf_only = email(&[("X-Spam-Test", "SPF_PASS")], "hi");
    assert!(!spf_only.is_verified(None));

    let neither = email(&[], "hi");
    assert!(!neither.is_verified(None));
  }

  #[test]
  fn the_trusted_upstream_result_wins_over_postmarks_own() {
    // What forwarded mail actually looks like: the sender is fine, but Postmark
    // checked SPF against the forwarder and failed it.
    let forwarded = email(
      &[
        ("X-Spam-Test", "DKIM_VALID"),
        (
          "Authentication-Results",
          "mx.google.com; dkim=pass header.i=@example.org; spf=pass (google.com: domain of leo@example.org designates 1.2.3.4 as permitted sender) smtp.mailfrom=leo@example.org; dmarc=pass header.from=example.org",
        ),
      ],
      "hi",
    );
    assert!(forwarded.is_verified(TRUSTED));

    // And a genuine failure upstream is still a failure, even though Postmark
    // happens to be happy.
    let failed_upstream = email(
      &[
        ("X-Spam-Test", "SPF_PASS,DKIM_VALID"),
        (
          "Authentication-Results",
          "mx.google.com; dkim=fail header.i=@example.org; spf=softfail smtp.mailfrom=leo@example.org",
        ),
      ],
      "hi",
    );
    assert!(!failed_upstream.is_verified(TRUSTED));
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
          "mx.google.com; dkim=fail; spf=fail smtp.mailfrom=attacker@example.org",
        ),
        (
          "Authentication-Results",
          "mx.google.com; dkim=pass; spf=pass smtp.mailfrom=someone@example.org",
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
    // Falls back to Postmark's own check, which has nothing to say here.
    assert!(!elsewhere.is_verified(TRUSTED));

    // With no upstream trusted at all, the header is never consulted.
    let trusted_header = email(
      &[(
        "Authentication-Results",
        "mx.google.com; dkim=pass; spf=pass",
      )],
      "hi",
    );
    assert!(!trusted_header.is_verified(None));
  }

  #[test]
  fn comments_in_the_header_cannot_fake_a_result() {
    // The parenthesised comment contains both a `;` and the text `spf=pass`,
    // and the real result is a failure.
    let value = "mx.google.com; spf=fail (google.com: sender is not permitted; tried spf=pass) smtp.mailfrom=a@example.org; dkim=fail";
    let stripped = strip_comments(value);
    assert_eq!(method_result(&stripped, "spf").as_deref(), Some("fail"));
    assert_eq!(method_result(&stripped, "dkim").as_deref(), Some("fail"));

    let email = email(&[("Authentication-Results", value)], "hi");
    assert!(!email.is_verified(TRUSTED));
  }

  #[test]
  fn method_names_are_matched_whole() {
    let value = strip_comments("mx.google.com; dkim-atps=pass; dkim=fail");
    // `dkim-atps` must not be read as `dkim`.
    assert_eq!(method_result(&value, "dkim").as_deref(), Some("fail"));
  }

  #[test]
  fn the_authserv_id_may_carry_a_version() {
    let email = email(
      &[(
        "Authentication-Results",
        "mx.google.com 1; dkim=pass; spf=pass",
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
  fn stripped_reply_is_preferred_over_the_quoted_body() {
    let mut email = email(&[], "hi");
    email.text_body = Some("new text\n\n> old text".to_owned());
    email.stripped_text_reply = Some("new text".to_owned());
    assert_eq!(email.body(), "new text");

    // An empty stripped reply means the client sent nothing usable; fall back.
    email.stripped_text_reply = Some("   ".to_owned());
    assert_eq!(email.body(), "new text\n\n> old text");
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

  #[test]
  fn secret_comparison_matches_only_identical_secrets() {
    assert!(secret_eq("hunter2", "hunter2"));
    assert!(!secret_eq("hunter2", "hunter3"));
    assert!(!secret_eq("hunter2", "hunter22"));
    assert!(!secret_eq("", "x"));
  }
}

#[cfg(test)]
mod integration {
  use crate::api::ApiTicket;
  use crate::api::ApiTicketActor;
  use crate::db::TicketStatus;
  use crate::util::test::ApiResultExt;
  use crate::util::test::TEST_POSTMARK_WEBHOOK_PASSWORD;
  use crate::util::test::TestSetup;
  use base64::Engine;
  use hyper::StatusCode;
  use serde_json::Value;
  use serde_json::json;

  fn basic_auth(password: &str) -> hyper::header::HeaderValue {
    let encoded = base64::engine::general_purpose::STANDARD
      .encode(format!("webhook:{password}"));
    format!("Basic {encoded}").try_into().unwrap()
  }

  /// A Postmark inbound payload, with only the fields the handler reads.
  fn inbound(message_id: &str, subject: &str, headers: Value) -> Value {
    let mut all = vec![json!({ "Name": "Message-ID", "Value": message_id })];
    all.extend(headers.as_array().cloned().unwrap_or_default());
    json!({
      "FromFull": { "Email": "someone@example.com", "Name": "Someone" },
      "Subject": subject,
      "TextBody": "I cannot publish my package.",
      "Headers": all,
      "Attachments": [],
    })
  }

  async fn deliver(
    t: &mut TestSetup,
    payload: Value,
  ) -> hyper::Response<hyper::Body> {
    t.http()
      .post("/api/hooks/postmark")
      .token(None)
      .header(
        hyper::header::AUTHORIZATION,
        basic_auth(TEST_POSTMARK_WEBHOOK_PASSWORD),
      )
      .body_json(payload)
      .call()
      .await
      .unwrap()
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
  ) -> Vec<crate::db::TicketMessage> {
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

    let resp =
      deliver(&mut t, inbound("<a@example.com>", "Help", json!([]))).await;
    assert_eq!(resp.status(), StatusCode::OK);

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

    let payload = inbound("<dup@example.com>", "Help", json!([]));
    deliver(&mut t, payload.clone()).await;
    // Postmark retries until it gets a 2xx, so a repeat must not 500 either.
    let resp = deliver(&mut t, payload).await;
    assert_eq!(resp.status(), StatusCode::OK);

    assert_eq!(all_tickets(&mut t).await.len(), 1);
  }

  #[tokio::test]
  async fn reply_threads_onto_the_ticket_it_answers() {
    let mut t = TestSetup::new().await;

    deliver(&mut t, inbound("<first@example.com>", "Help", json!([]))).await;
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
      &mut t,
      inbound(
        "<second@example.com>",
        "Re: Help",
        json!([{ "Name": "In-Reply-To", "Value": auto_reply_id }]),
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

    deliver(&mut t, inbound("<first@example.com>", "Help", json!([]))).await;
    let ticket = all_tickets(&mut t).await.remove(0);

    // No In-Reply-To or References at all: the ticket number in the subject is
    // the only thing tying this message to the conversation.
    deliver(
      &mut t,
      inbound(
        "<second@example.com>",
        &format!("Re: [{}] Support request", ticket.ticket_number),
        json!([]),
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

    deliver(&mut t, inbound("<first@example.com>", "Help", json!([]))).await;
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
      &mut t,
      inbound(
        "<second@example.com>",
        &format!("Re: [{}] Support request", ticket.ticket_number),
        json!([]),
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
    let mut payload = inbound("<a@example.com>", "Help", json!([]));
    payload["Attachments"] = json!([{
      "Name": "log.txt",
      "Content": base64::engine::general_purpose::STANDARD.encode(content),
      "ContentType": "text/plain",
      "ContentLength": content.len(),
    }]);
    deliver(&mut t, payload).await;

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

    let body = hyper::body::to_bytes(resp.body_mut()).await.unwrap();
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

    deliver(&mut t, inbound("<a@example.com>", "Help", json!([]))).await;
    let ticket = all_tickets(&mut t).await.remove(0);

    // No Postmark client is configured under test, so nothing is queued and
    // the ticket still lands. What matters is that the webhook succeeded and
    // the auto-reply is recorded on the ticket either way.
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
      .enqueue_email(crate::db::NewEmailDelivery {
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

    db.mark_email_delivery_sent(id).await.unwrap();

    // Once sent it is invisible to both the delivery path and the sweeper, so a
    // task Cloud Tasks redelivers cannot produce a second email.
    assert!(db.get_pending_email_delivery(id).await.unwrap().is_none());
    assert!(
      db.list_stale_email_deliveries(0, 10)
        .await
        .unwrap()
        .is_empty()
    );
  }

  #[tokio::test]
  async fn a_delivery_is_not_lost_when_no_sender_is_configured() {
    let t = TestSetup::new().await;
    let db = t.ephemeral_database.database.as_ref().unwrap();

    let id = db
      .enqueue_email(crate::db::NewEmailDelivery {
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

    // The queue dispatches to a service that has no Postmark credential. This
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
      .enqueue_email(crate::db::NewEmailDelivery {
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

    deliver(&mut t, inbound("<a@example.com>", "Help", json!([]))).await;
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
    assert_eq!(reply.direction, crate::db::TicketMessageDirection::Outbound);

    // Staff spoke last, so the reporter is the one now owed a response.
    let after = all_tickets(&mut t).await.remove(0);
    assert_eq!(after.status, TicketStatus::WaitingOnUser);
  }

  #[tokio::test]
  async fn a_staff_note_is_never_shown_to_the_reporter() {
    let mut t = TestSetup::new().await;

    deliver(&mut t, inbound("<a@example.com>", "Help", json!([]))).await;
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

    deliver(&mut t, inbound("<a@example.com>", "Help", json!([]))).await;
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
        "kind": crate::db::TicketKind::Other,
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
    assert_eq!(note.direction, crate::db::TicketMessageDirection::Outbound);
  }

  #[tokio::test]
  async fn a_note_cannot_be_written_by_anyone_but_staff() {
    let mut t = TestSetup::new().await;

    deliver(&mut t, inbound("<a@example.com>", "Help", json!([]))).await;
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
        "kind": crate::db::TicketKind::Other,
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

    deliver(&mut t, inbound("<a@example.com>", "Help", json!([]))).await;
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
  async fn wrong_webhook_password_is_rejected() {
    let mut t = TestSetup::new().await;

    let mut resp = t
      .http()
      .post("/api/hooks/postmark")
      .token(None)
      .header(hyper::header::AUTHORIZATION, basic_auth("wrong"))
      .body_json(inbound("<a@example.com>", "Help", json!([])))
      .call()
      .await
      .unwrap();
    resp.expect_err(StatusCode::UNAUTHORIZED).await;

    assert!(all_tickets(&mut t).await.is_empty());
  }

  #[tokio::test]
  async fn email_without_a_message_id_is_refused() {
    let mut t = TestSetup::new().await;

    let mut payload = inbound("<a@example.com>", "Help", json!([]));
    payload["Headers"] = json!([]);

    let mut resp = deliver(&mut t, payload).await;
    resp.expect_err(StatusCode::BAD_REQUEST).await;

    assert!(all_tickets(&mut t).await.is_empty());
  }

  #[tokio::test]
  async fn claim_token_grants_access_and_binds_the_ticket() {
    let mut t = TestSetup::new().await;

    deliver(&mut t, inbound("<a@example.com>", "Help", json!([]))).await;
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
