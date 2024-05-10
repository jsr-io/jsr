// Copyright 2024 the JSR authors. All rights reserved. MIT license.
use hyper::Body;
use hyper::Response;
use std::borrow::Cow;

use crate::errors;
use crate::gcp::GcsError;

use super::ApiPublishingTask;

errors!(
  TarballSizeLimitExceeded {
    status: PAYLOAD_TOO_LARGE,
    fields: { size: u64, max_size: u64 },
    ({ size, max_size }) => "The uploaded tarball ({size} bytes) exceeds the maximum allowed size ({max_size} bytes).",
  },
  // TODO: re-implement
  // PublishMaxFileSizeExceeded {
  //   status: PAYLOAD_TOO_LARGE,
  //   fields: { size: u64, max_size: u64 },
  //   ({ size, max_size }) => "The uploaded file ({size} bytes) exceeds the maximum allowed size ({max_size} bytes).",
  // },
  MissingGzipContentEncoding {
    status: BAD_REQUEST,
    "The uploaded content must be gzip encoded, and the content-encoding header must be set to 'gzip'.",
  },
  PublishNotFound {
    status: NOT_FOUND,
    "The requested publish was not found.",
  },
  UserNotFound {
    status: NOT_FOUND,
    "The requested user was not found. Only users who have logged in to JSR at least once are visible.",
  },
  ScopeNotFound {
    status: NOT_FOUND,
    "The requested scope was not found.",
  },
  PackageNotFound {
    status: NOT_FOUND,
    "The requested package was not found.",
  },
  PackageVersionNotFound {
    status: NOT_FOUND,
    "The requested package version was not found.",
  },
  EntrypointOrSymbolNotFound {
    status: NOT_FOUND,
    "The requested entrypoint or symbol was not found.",
  },
  PackagePathNotFound {
    status: NOT_FOUND,
    "The requested path was not found.",
  },
  TokenNotFound {
    status: NOT_FOUND,
    "The requested token was not found.",
  },
  InternalServerError {
    status: INTERNAL_SERVER_ERROR,
    "Internal Server Error",
  },
  MalformedRequest {
    status: BAD_REQUEST,
    fields: { msg: Cow<'static, str> },
    ({ msg }) => "Malformed request: {msg}.",
  },
  GithubOauthError {
    status: BAD_REQUEST,
    fields: { msg: String },
    ({ msg }) => "GitHub authentication failed: {msg}.",
  },
  InvalidOauthState {
    status: BAD_REQUEST,
    "Invalid OAuth State.",
  },
  Blocked {
    status: FORBIDDEN,
    "Your account is blocked.",
  },
  DuplicateVersionPublish {
    status: BAD_REQUEST,
    fields: { task: Box<ApiPublishingTask> },
    data_fields: { task },
    "This version of the package is already being published.",
  },
  WeeklyPublishAttemptsLimitExceeded {
    status: BAD_REQUEST,
    fields: { limit: i32 },
    ({ limit }) => "Exceeded weekly limit of {limit} publish attempts for scope.",
  },
  WeeklyPackageLimitExceeded {
    status: BAD_REQUEST,
    fields: { limit: i32 },
    ({ limit }) => "Exceeded weekly limit of {limit} new packages for scope.",
  },
  PackageLimitExceeded {
    status: BAD_REQUEST,
    fields: { limit: i32 },
    ({ limit }) => "Exceeded limit of {limit} new packages for scope.",
  },

  ScopeAlreadyExists {
    status: CONFLICT,
    "A scope with this or a very similar name already exists.",
  },
  PackageAlreadyExists {
    status: CONFLICT,
    "A package with this or a very similar name already exists.",
  },
  AlreadyInvited {
    status: BAD_REQUEST,
    "This user has already been invited to this scope.",
  },
  AlreadyScopeMember {
    status: BAD_REQUEST,
    "This user is already a member of this scope.",
  },
  ScopeMemberNotFound {
    status: NOT_FOUND,
    "The requested scope member was not found.",
  },
  ScopeMustHaveAdmin {
    status: BAD_REQUEST,
    "The last scope admin can not be removed / downgraded.",
  },
  NoScopeOwnerAvailable {
    status: BAD_REQUEST,
    "All other scope admins have exceeded their scope limits, so none can be made owner of the scope.",
  },
  ScopeLimitReached {
    status: BAD_REQUEST,
    "This account has reached the maximum number of created scopes.",
  },
  ScopeInviteNotFound {
    status: NOT_FOUND,
    "The requested scope invite was not found.",
  },
  GithubSamlEnforcement {
    status: BAD_REQUEST,
    "To access this repository, GitHub requires SAML SSO. Please follow this guide to be able to link this repository to JSR: https://docs.github.com/en/enterprise-cloud@latest/apps/using-github-apps/saml-and-github-apps",
  },
  GithubRepositoryNotFound {
    status: NOT_FOUND,
    "The requested GitHub repository was not found. Make sure the repository exists and is public.",
  },
  GithubRepositoryNotPublic {
    status: BAD_REQUEST,
    "To link a GitHub repository, it must be public.",
  },
  GithubRepositoryNotAuthorized {
    status: BAD_REQUEST,
    "To link a GitHub repository, you must have at least push permissions for it.",
  },
  MissingPermission {
    status: FORBIDDEN,
    "The credential this request was authenticated with does not have the necessary permissions to perform this action.",
  },
  ActorNotAuthorized {
    status: FORBIDDEN,
    "The actor that this request was authenticated for is not authorized to access this resource.",
  },
  CredentialNotInteractive {
    status: FORBIDDEN,
    "The credential that this request was authenticated for is not interactive (a web token).",
  },
  ActorNotUser {
    status: FORBIDDEN,
    "The actor that this request was authenticated for is not a user.",
  },
  ActorNotScopeAdmin {
    status: FORBIDDEN,
    "The actor that this request was authenticated for is not authorized as a scope admin for this scope.",
  },
  ActorNotScopeMember {
    status: FORBIDDEN,
    "The actor that this request was authenticated for is not authorized as a scope member for this scope.",
  },
  ScopeRequiresPublishingFromCI {
    status: FORBIDDEN,
    "This scope requires that all packages must be published from CI.",
  },
  InvalidBearerToken {
    status: UNAUTHORIZED,
    "The provided bearer token is invalid.",
  },
  InvalidOidcToken {
    status: UNAUTHORIZED,
    fields: { msg: Cow<'static, str> },
    ({ msg }) => "The provided OIDC token is invalid: {msg}",
  },
  MissingAuthentication {
    status: UNAUTHORIZED,
    "This request requires authentication.",
  },
  AuthorizationNotFound {
    status: NOT_FOUND,
    "The requested authorization was not found.",
  },
  AuthorizationExpired {
    status: BAD_REQUEST,
    "The requested authorization has expired.",
  },
  AuthorizationInvalidVerifier {
    status: BAD_REQUEST,
    "The passed verifier does not match the authorization's challenge.",
  },
  AuthorizationDenied {
    status: BAD_REQUEST,
    "The requested authorization has been denied by the user.",
  },
  AuthorizationPending { // it is important that this code is never changed!
    status: BAD_REQUEST,
    "The requested authorization is still pending. Try again later.",
  },
  ScopeNotEmpty {
    status: CONFLICT,
    "The requested scope contains packages. Only empty scopes may be deleted.",
  },
  PackageNotEmpty {
    status: CONFLICT,
    "The requested package has a version published, or is currently publishing a version. Only empty packages may be deleted.",
  },
  ScopeNameNotAllowed {
    status: BAD_REQUEST,
    "The provided scope name is not allowed.",
  },
  ScopeNameReserved {
    status: BAD_REQUEST,
    "The provided scope name is reserved. If you want to claim it, please contact help@jsr.io.",
  },
  PackageNameNotAllowed {
    status: BAD_REQUEST,
    "The provided package name is not allowed.",
  },
);

pub fn map_unique_violation(err: sqlx::Error, new_err: ApiError) -> ApiError {
  if let Some(db_err) = err.as_database_error() {
    if let Some(code) = db_err.code() {
      // Code 23505 is unique_violation.
      // See https://www.postgresql.org/docs/13/errcodes-appendix.html
      if code == "23505" {
        return new_err;
      }
    }
  }
  err.into()
}

impl From<anyhow::Error> for ApiError {
  fn from(error: anyhow::Error) -> ApiError {
    eprintln!("internal server error: {:?}", error);
    ApiError::InternalServerError
  }
}

impl From<sqlx::Error> for ApiError {
  fn from(error: sqlx::Error) -> ApiError {
    anyhow::Error::from(error).into()
  }
}

impl From<std::str::Utf8Error> for ApiError {
  fn from(error: std::str::Utf8Error) -> ApiError {
    anyhow::Error::from(error).into()
  }
}

impl From<hyper::http::uri::InvalidUriParts> for ApiError {
  fn from(error: hyper::http::uri::InvalidUriParts) -> ApiError {
    anyhow::Error::from(error).into()
  }
}

impl From<hyper::http::uri::InvalidUri> for ApiError {
  fn from(error: hyper::http::uri::InvalidUri) -> ApiError {
    anyhow::Error::from(error).into()
  }
}

impl From<serde_json::Error> for ApiError {
  fn from(error: serde_json::Error) -> ApiError {
    anyhow::Error::from(error).into()
  }
}

impl From<oauth2::reqwest::Error<reqwest::Error>> for ApiError {
  fn from(error: oauth2::reqwest::Error<reqwest::Error>) -> ApiError {
    anyhow::Error::from(error).into()
  }
}

impl
  From<
    oauth2::RequestTokenError<
      oauth2::reqwest::Error<reqwest::Error>,
      oauth2::basic::BasicErrorResponse,
    >,
  > for ApiError
{
  fn from(
    error: oauth2::RequestTokenError<
      oauth2::reqwest::Error<reqwest::Error>,
      oauth2::basic::BasicErrorResponse,
    >,
  ) -> ApiError {
    anyhow::Error::from(error).into()
  }
}

impl
  From<
    oauth2::RequestTokenError<
      oauth2::reqwest::Error<reqwest::Error>,
      oauth2::DeviceCodeErrorResponse,
    >,
  > for ApiError
{
  fn from(
    error: oauth2::RequestTokenError<
      oauth2::reqwest::Error<reqwest::Error>,
      oauth2::DeviceCodeErrorResponse,
    >,
  ) -> ApiError {
    anyhow::Error::from(error).into()
  }
}

impl From<oauth2::RequestTokenError<ApiError, oauth2::DeviceCodeErrorResponse>>
  for ApiError
{
  fn from(
    error: oauth2::RequestTokenError<ApiError, oauth2::DeviceCodeErrorResponse>,
  ) -> ApiError {
    match error {
      oauth2::RequestTokenError::Request(e) => e,
      e => anyhow::Error::from(e).into(),
    }
  }
}

impl From<oauth2::ConfigurationError> for ApiError {
  fn from(error: oauth2::ConfigurationError) -> ApiError {
    anyhow::Error::from(error).into()
  }
}

impl From<GcsError> for ApiError {
  fn from(error: GcsError) -> ApiError {
    anyhow::Error::from(error).into()
  }
}

impl From<std::io::Error> for ApiError {
  fn from(error: std::io::Error) -> ApiError {
    anyhow::Error::from(error).into()
  }
}
