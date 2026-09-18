// Copyright 2024 the JSR authors. All rights reserved. MIT license.
use shared::errors::SharedError;
use std::borrow::Cow;

shared::errors!(
  TaskError:
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
    anyhow::Error::from(error).into()
  }
}

impl From<anyhow::Error> for TaskError {
  fn from(error: anyhow::Error) -> TaskError {
    eprintln!("internal server error: {:?}", error);
    SharedError::InternalServerError.into()
  }
}

impl From<sqlx::Error> for TaskError {
  fn from(error: sqlx::Error) -> TaskError {
    anyhow::Error::from(error).into()
  }
}

impl From<serde_json::Error> for TaskError {
  fn from(error: serde_json::Error) -> TaskError {
    anyhow::Error::from(error).into()
  }
}

pub type TaskResult<D> = Result<D, TaskError>;

pub async fn error_handler(
  err: routerify::RouteError,
  _: routerify::RequestInfo,
) -> hyper::Response<hyper::Body> {
  // Because `routerify::RouteError` is a boxed error, it must be downcast
  // first. Unwrap for simplicity.
  let api_err = err.downcast::<TaskError>().unwrap();
  let is_server_error = api_err.status_code().is_server_error();
  let span = tracing::Span::current();
  span.record(
    "otel.status_code",
    if is_server_error { "error" } else { "ok" },
  );
  // Only server errors (5xx) are logged. Client errors (4xx, e.g. 404s) are
  // expected and would otherwise pollute error logs and trace error rates. The
  // route handlers no longer carry `err` on their `#[instrument]`, so this is
  // the single place a directly-constructed 5xx `ApiError` gets surfaced;
  // anyhow-backed 5xx are additionally logged at their origin (leaf spans keep
  // `err`).
  if is_server_error {
    tracing::error!(error = %api_err, status = api_err.status_code().as_u16());
  }
  api_err.json_response()
}
