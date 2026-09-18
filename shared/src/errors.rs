// Copyright 2024 the JSR authors. All rights reserved. MIT license.
use std::borrow::Cow;

crate::errors!(
  SharedError:
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

impl From<anyhow::Error> for SharedError {
  fn from(error: anyhow::Error) -> SharedError {
    eprintln!("internal server error: {:?}", error);
    SharedError::InternalServerError.into()
  }
}
