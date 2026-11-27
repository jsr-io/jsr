// Copyright 2024 the JSR authors. All rights reserved. MIT license.
use shared::errors::SharedError;
use shared::s3::S3Error;
use std::borrow::Cow;

shared::errors!(
  TaskError:
  InternalServerError {
    status: INTERNAL_SERVER_ERROR,
    "Internal Server Error",
  },
  MalformedRequest {
    status: BAD_REQUEST,
    fields: { msg: Cow<'static, str> },
    ({ msg }) => "Malformed request: {msg}.",
  },
  PackageVersionNotFound {
    status: NOT_FOUND,
    "The requested package version was not found.",
  },
  PublishNotFound {
    status: NOT_FOUND,
    "The requested publish was not found.",
  },
  MissingTurnstileToken {
    status: BAD_REQUEST,
    "The captcha was not completed. Please complete the captcha and try again.",
  },
  InvalidTurnstileToken {
    status: BAD_REQUEST,
    "The captcha response was invalid or has already been used. Please try again.",
  },
  TurnstileVerificationFailed {
    status: SERVICE_UNAVAILABLE,
    "The captcha could not be verified at this time. Please try again shortly.",
  },
);

impl From<SharedError> for TaskError {
  fn from(error: SharedError) -> TaskError {
    match error {
      SharedError::InternalServerError => TaskError::InternalServerError,
      SharedError::MalformedRequest { msg } => {
        TaskError::MalformedRequest { msg }
      }
      SharedError::PackageVersionNotFound => TaskError::PackageVersionNotFound,
      SharedError::PublishNotFound => TaskError::PublishNotFound,
      SharedError::MissingTurnstileToken => TaskError::MissingTurnstileToken,
      SharedError::InvalidTurnstileToken => TaskError::InvalidTurnstileToken,
      SharedError::TurnstileVerificationFailed => {
        TaskError::TurnstileVerificationFailed
      }
    }
  }
}

impl From<anyhow::Error> for TaskError {
  fn from(error: anyhow::Error) -> TaskError {
    eprintln!("internal server error: {:?}", error);
    TaskError::InternalServerError
  }
}

impl From<shared::db::DbError> for TaskError {
  fn from(error: shared::db::DbError) -> TaskError {
    anyhow::Error::from(error).into()
  }
}

impl From<serde_json::Error> for TaskError {
  fn from(error: serde_json::Error) -> TaskError {
    anyhow::Error::from(error).into()
  }
}

impl From<S3Error> for TaskError {
  fn from(error: S3Error) -> TaskError {
    anyhow::Error::from(error).into()
  }
}

pub type TaskResult<D> = Result<D, TaskError>;

impl axum::response::IntoResponse for TaskError {
  fn into_response(self) -> axum::response::Response {
    let is_server_error = self.status_code().is_server_error();
    let span = tracing::Span::current();
    span.record(
      "otel.status_code",
      if is_server_error { "error" } else { "ok" },
    );
    // Only server errors (5xx) are logged. Client errors (4xx) are expected and
    // would otherwise pollute error logs and trace error rates. anyhow-backed
    // 5xx are additionally logged at their origin (leaf spans keep `err`).
    if is_server_error {
      tracing::error!(error = %self, status = self.status_code().as_u16());
    }
    self.json_response()
  }
}
