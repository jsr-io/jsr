// Copyright 2024 the JSR authors. All rights reserved. MIT license.
use postmark::Query;
use postmark::reqwest::PostmarkClient;
use tracing::instrument;

pub trait Email {
  fn subject(&self) -> String;
  fn render(&self) -> Result<EmailContent, anyhow::Error>;
}

#[derive(Debug)]
pub struct EmailContent {
  pub text: String,
  pub html: String,
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

  /// Builds a sender that delivers through Postmark's HTTP API with the given
  /// server token.
  pub fn from_postmark_token(
    token: String,
    from: String,
    from_name: String,
  ) -> Self {
    Self::new(
      PostmarkClient::builder().server_token(token).build(),
      from,
      from_name,
    )
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

/// The queue that drives `/tasks/send_email`. `None` outside the worker, where deliveries are sent inline instead (see [`enqueue`]).
pub struct EmailQueue(pub Option<crate::queue::Queue>);

/// How many times a delivery is attempted before it is abandoned. The queue
/// does its own retrying on top of this; the count here is the backstop that
/// stops a permanently undeliverable address being retried forever.
pub const MAX_EMAIL_ATTEMPTS: i32 = 8;

/// Queues an email for delivery.
///
/// The row is committed first and the queue is nudged afterwards, so a failure
/// to reach the queue costs a delay rather than the email: the sweeper in
/// `tasks.rs` re-drives anything that never reached a terminal state. Errors
/// from the nudge are logged and swallowed for that reason — the caller's own
/// request must not fail because of an email.
///
/// With no queue configured (local development, tests) the delivery is sent
/// inline so behaviour matches production as closely as it can without one.
#[instrument(name = "emails::enqueue", skip_all, err, fields(delivery_id))]
pub async fn enqueue<T: Email>(
  db: &crate::db::Database,
  email_sender: &EmailSender,
  queue: &EmailQueue,
  to: String,
  args: T,
  thread: Option<EmailThread<'_>>,
) -> Result<uuid::Uuid, anyhow::Error> {
  let subject = args.subject();
  let content = args.render()?;

  let (message_id, in_reply_to, reference_ids) = match thread {
    Some(thread) => (
      Some(thread.message_id.to_owned()),
      thread.in_reply_to,
      thread.references,
    ),
    None => (None, None, Vec::new()),
  };

  let id = db
    .enqueue_email(crate::models::NewEmailDelivery {
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
      if let Err(err) = queue.send(body.into()).await {
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
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct SendEmailTask {
  pub id: uuid::Uuid,
}

/// Attempts one delivery, recording the outcome against the row.
///
/// Returns `Ok(true)` when the delivery reached a terminal state (sent, already
/// sent, or abandoned) and must not be retried, and `Ok(false)` when it failed
/// but is worth another attempt. The caller turns the latter into a non-2xx so
/// the queue retries.
#[instrument(name = "emails::deliver", skip(db, email_sender), err)]
pub async fn deliver(
  db: &crate::db::Database,
  email_sender: Option<&EmailSender>,
  id: uuid::Uuid,
) -> Result<bool, anyhow::Error> {
  let Some(delivery) = db.get_pending_email_delivery(id).await? else {
    // Already sent or abandoned. The queue redelivering a task it has already
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
