// Copyright 2024 the JSR authors. All rights reserved. MIT license.

use crate::api::ApiError;
use serde::Deserialize;
use serde::Serialize;
use tracing::error;
use tracing::instrument;
use tracing::warn;

#[derive(Clone)]
pub struct AnalyticsEngineClient {
  account_id: String,
  api_token: String,
}

/// Client for Cloudflare Turnstile's `siteverify` endpoint, which exchanges the
/// response token minted by the widget on the login page for a verdict on
/// whether the client that produced it looks like a browser.
#[derive(Clone)]
pub struct TurnstileClient {
  secret_key: String,
}

/// Wrapper around an optional `TurnstileClient` so it can be stored in the
/// routerify data map alongside other shared services. A `None` value means no
/// secret key was configured (local dev, tests), and the captcha check is
/// skipped — matching the frontend, which only renders the widget when it has a
/// site key.
#[derive(Clone)]
pub struct Turnstile(pub Option<TurnstileClient>);

#[derive(Debug, Deserialize)]
struct SiteverifyResponse {
  success: bool,
  /// Why the token was rejected, e.g. `invalid-input-response` for a malformed
  /// token or `timeout-or-duplicate` for an expired or replayed one. Logged
  /// rather than surfaced, since the user can act on neither.
  ///
  /// A bad *secret* does not appear here: siteverify answers `400` for that,
  /// which `siteverify` reports as an error instead.
  #[serde(default, rename = "error-codes")]
  error_codes: Vec<String>,
}

impl Turnstile {
  /// Check a captcha response token, if verification is enabled.
  ///
  /// Unlike cache purging, this is not best-effort: a token that cannot be
  /// verified is rejected rather than waved through, because the whole point of
  /// the check is to stop a caller that will not produce a valid one.
  ///
  /// A rejected token is the user's problem and yields a `400`. Anything that
  /// stops us from reaching a verdict — a siteverify outage, or a secret key so
  /// wrong that siteverify answers `400` — is ours, and fails login closed with
  /// a retryable `503` rather than blaming the user's captcha.
  pub async fn verify(&self, token: Option<&str>) -> Result<(), ApiError> {
    let Some(client) = &self.0 else {
      return Ok(());
    };

    let token = token
      .filter(|token| !token.is_empty())
      .ok_or(ApiError::MissingTurnstileToken)?;

    match client.siteverify(token).await {
      Ok(true) => Ok(()),
      Ok(false) => Err(ApiError::InvalidTurnstileToken),
      Err(_) => Err(ApiError::TurnstileVerificationFailed),
    }
  }
}

impl TurnstileClient {
  pub fn new(secret_key: String) -> Self {
    Self { secret_key }
  }

  /// Returns whether Cloudflare considers `token` a valid, unspent response for
  /// our site key. A token is single-use, so a replay verifies as `false`.
  ///
  /// Errors on a non-2xx status, which siteverify uses for a request it could
  /// not evaluate at all — in practice, a missing or malformed secret key.
  #[instrument(
    name = "cloudflare.turnstile_siteverify",
    skip(self, token),
    err
  )]
  async fn siteverify(&self, token: &str) -> Result<bool, anyhow::Error> {
    let response = crate::util::shared_http_client()
      .post("https://challenges.cloudflare.com/turnstile/v0/siteverify")
      .form(&[("secret", self.secret_key.as_str()), ("response", token)])
      .send()
      .await?;

    if !response.status().is_success() {
      let status = response.status();
      let body = response.text().await.unwrap_or_default();
      error!(
        "Cloudflare Turnstile siteverify failed (status={}): {}",
        status, body
      );
      return Err(anyhow::anyhow!(
        "Cloudflare Turnstile siteverify failed (status={}): {}",
        status,
        body,
      ));
    }

    let result: SiteverifyResponse = response.json().await?;
    if !result.success {
      warn!(
        "Cloudflare Turnstile rejected a token: {:?}",
        result.error_codes
      );
    }

    Ok(result.success)
  }
}

/// Client for the Cloudflare zone cache-purge endpoint, used to invalidate
/// cached package and npm version manifests after a publish or mutation.
///
/// Construction requires both a zone ID and an API token; if either is
/// missing the API server simply does not build a client and all purge
/// calls become no-ops.
#[derive(Clone)]
pub struct CachePurgeClient {
  zone_id: String,
  api_token: String,
}

/// Wrapper around an optional `CachePurgeClient` so it can be stored in
/// the routerify data map alongside other shared services. A `None`
/// value means cache purging is disabled (e.g. local dev), and call
/// sites should treat it as a no-op.
#[derive(Clone)]
pub struct CachePurge(pub Option<CachePurgeClient>);

impl CachePurge {
  /// Purge `urls` if a client is configured. Errors are logged inside
  /// `purge_urls` and converted into `Ok(())` here, since callers want
  /// best-effort behaviour (the manifests have `stale-while-revalidate`
  /// as their durability net).
  pub async fn purge(&self, urls: Vec<String>) {
    let Some(client) = &self.0 else {
      return;
    };
    let _ = client.purge_urls(urls).await;
  }
}

impl CachePurgeClient {
  pub fn new(zone_id: String, api_token: String) -> Self {
    Self { zone_id, api_token }
  }

  /// Purge a set of fully-qualified URLs from the Cloudflare zone cache.
  ///
  /// Errors are logged and returned — callers should treat purge as
  /// best-effort and not fail the publish on a purge failure (the
  /// `stale-while-revalidate` window on the manifests is the safety net).
  #[instrument(name = "cloudflare.purge_cache", skip(self, urls), err)]
  pub async fn purge_urls(
    &self,
    urls: Vec<String>,
  ) -> Result<(), anyhow::Error> {
    if urls.is_empty() {
      return Ok(());
    }

    let body = serde_json::json!({ "files": urls });
    let response = crate::util::shared_http_client()
      .post(format!(
        "https://api.cloudflare.com/client/v4/zones/{}/purge_cache",
        self.zone_id,
      ))
      .bearer_auth(&self.api_token)
      .json(&body)
      .send()
      .await?;

    if !response.status().is_success() {
      let status = response.status();
      let body = response.text().await.unwrap_or_default();
      error!(
        "Cloudflare cache purge failed (status={}): {}",
        status, body
      );
      return Err(anyhow::anyhow!(
        "Cloudflare cache purge failed (status={}): {}",
        status,
        body,
      ));
    }

    Ok(())
  }
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AnalyticsQueryResult {
  pub data: Vec<DownloadRecord>,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct DownloadRecord {
  pub time_bucket: String,
  pub scope: String,
  pub package: String,
  // because 'version' is reserved in cloudflare analytics engine
  pub ver: String,
  pub count: String,
}

impl AnalyticsEngineClient {
  pub fn new(account_id: String, api_token: String) -> Self {
    Self {
      account_id,
      api_token,
    }
  }

  pub async fn query_downloads(
    &self,
    query: String,
  ) -> Result<Vec<DownloadRecord>, anyhow::Error> {
    let response = crate::util::shared_http_client()
      .post(format!(
        "https://api.cloudflare.com/client/v4/accounts/{}/analytics_engine/sql",
        self.account_id,
      ))
      .bearer_auth(&self.api_token)
      .body(query)
      .send()
      .await?;

    if !response.status().is_success() {
      let status = response.status();
      let body = response.text().await?;
      error!(
        "Cloudflare Analytics Engine query failed (status={}): {}",
        status, body
      );
      return Err(anyhow::anyhow!(
        "Cloudflare Analytics Engine query failed: {}",
        body
      ));
    }

    let result: AnalyticsQueryResult = response.json().await?;

    Ok(result.data)
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  // Without a secret key the check is disabled end to end, so a request that
  // carries no token (as the frontend sends when it has no site key) is fine.
  #[tokio::test]
  async fn turnstile_disabled_accepts_missing_token() {
    assert!(Turnstile(None).verify(None).await.is_ok());
  }

  // When enabled, a request with no token must be rejected before any network
  // call to siteverify is attempted.
  #[tokio::test]
  async fn turnstile_enabled_rejects_missing_token() {
    let turnstile = Turnstile(Some(TurnstileClient::new("secret".into())));
    let err = turnstile.verify(None).await.unwrap_err();
    assert_eq!(err.code(), "missingTurnstileToken");
  }

  // A form submitted without solving the widget yields an empty field rather
  // than an absent one; that must be rejected the same way, and again without
  // asking Cloudflare about the empty string.
  #[tokio::test]
  async fn turnstile_enabled_rejects_empty_token() {
    let turnstile = Turnstile(Some(TurnstileClient::new("secret".into())));
    let err = turnstile.verify(Some("")).await.unwrap_err();
    assert_eq!(err.code(), "missingTurnstileToken");
  }
}
