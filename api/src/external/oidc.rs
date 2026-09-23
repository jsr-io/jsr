// Copyright 2024 the JSR authors. All rights reserved. MIT license.
//
// Generic OIDC token verification used by per-CI-provider modules. Provider-
// specific claim shapes (GitHub `repository_id`, GitLab `project_id`, etc.)
// stay in their respective `external/{provider}.rs` files; this module only
// handles JWKS fetching and JWT signature/issuer validation.

use crate::api::ApiError;
use crate::util::ApiResult;
use crate::util::shared_http_client;
use anyhow::Context;
use jsonwebtoken::jwk::JwkSet;
use serde::de::DeserializeOwned;
use tracing::error;
use tracing::instrument;

/// Identifies which OIDC provider issued a token. Adding a provider means
/// adding a variant here and a matching entry in [`Self::auth_prefix`],
/// [`Self::span_kind`], and [`Self::all`].
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum OidcProviderKind {
  GitHub,
}

impl OidcProviderKind {
  /// `Authorization` header prefix that selects this provider, including the
  /// trailing space (e.g. `"githuboidc "`).
  pub fn auth_prefix(self) -> &'static str {
    match self {
      OidcProviderKind::GitHub => "githuboidc ",
    }
  }

  /// Short identifier used in tracing spans and logs.
  pub fn span_kind(self) -> &'static str {
    match self {
      OidcProviderKind::GitHub => "githuboidc",
    }
  }

  pub fn all() -> &'static [OidcProviderKind] {
    &[OidcProviderKind::GitHub]
  }
}

/// Configuration for verifying tokens issued by a specific OIDC provider.
pub struct OidcProvider {
  pub kind: OidcProviderKind,
  pub issuer: String,
  pub jwks_url: String,
}

/// Fetch the provider's JWKS, verify the token's signature against it, and
/// validate that the token's `iss` matches `provider.issuer`. Provider-specific
/// claim fields are decoded into the caller-chosen `Claims` shape.
#[instrument(name = "oidc::verify_token", err, skip(provider, token), fields(oidc.kind = provider.kind.span_kind()))]
pub async fn verify_token<Claims: DeserializeOwned>(
  provider: &OidcProvider,
  token: &str,
) -> ApiResult<Claims> {
  let res = shared_http_client()
    .get(&provider.jwks_url)
    .header("Accept", "application/json")
    .send()
    .await
    .context("failed to download oidc jwks")?;
  let status = res.status();
  if !status.is_success() {
    let body = res.text().await.unwrap_or_default();
    error!(
      "failed to download oidc jwks for {}: {body} (status: {status}) ",
      provider.kind.span_kind()
    );
    return Err(ApiError::InternalServerError);
  }
  let jwks: JwkSet = res.json().await.context("failed to parse oidc jwks")?;

  let header = jsonwebtoken::decode_header(token).map_err(|err| {
    ApiError::InvalidOidcToken {
      msg: err.to_string().into(),
    }
  })?;
  let kid = header.kid.ok_or(ApiError::InvalidOidcToken {
    msg: "missing kid".into(),
  })?;

  let jwk = jwks.find(&kid).ok_or_else(|| ApiError::InvalidOidcToken {
    msg: format!("invalid kid: {kid}").into(),
  })?;

  let alg: jsonwebtoken::Algorithm = jwk
    .common
    .key_algorithm
    .ok_or_else(|| {
      error!("jwk {jwk:?} missing algorithm");
      ApiError::InternalServerError
    })?
    .try_into()
    .map_err(|err| {
      error!("jwk {jwk:?} has an unsupported algorithm: {err}");
      ApiError::InternalServerError
    })?;
  let decoding_key =
    jsonwebtoken::DecodingKey::from_jwk(jwk).map_err(|err| {
      error!("failed to build a decoding key from jwk {jwk:?}: {err}");
      ApiError::InternalServerError
    })?;
  let mut validation = jsonwebtoken::Validation::new(alg);
  validation.set_issuer(&[&provider.issuer]);
  // The `aud` claim is not checked, as before: jsonwebtoken 9+ rejects tokens
  // that carry an audience unless one is configured, whereas 8 (the previous
  // version) only checked `aud` when one was set.
  validation.validate_aud = false;
  let decoded =
    jsonwebtoken::decode::<Claims>(token, &decoding_key, &validation).map_err(
      |err| ApiError::InvalidOidcToken {
        msg: err.to_string().into(),
      },
    )?;

  Ok(decoded.claims)
}
