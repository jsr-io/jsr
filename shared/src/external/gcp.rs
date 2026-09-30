// Copyright 2024 the JSR authors. All rights reserved. MIT license.
// Copyright 2024 the JSR authors. All rights reserved. MIT license.
use bytes::Bytes;
use hyper::StatusCode;
use serde::Deserialize;
use std::collections::HashMap;
use std::str::FromStr;
use std::sync::Arc;
use std::sync::Mutex;
use std::time::Duration;
use std::time::Instant;
use tracing::instrument;

const HTTP_CONNECT_TIMEOUT: Duration = Duration::from_secs(10);

#[derive(Deserialize)]
pub struct AccessTokenResponse {
  access_token: String,
  expires_in: u64,
}

#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub enum MetadataStrategy {
  /// Get authentication information from the instance metadata server.
  InstanceMetadata,
  /// Returned fixed fake tokens for testing.
  Testing,
}

impl FromStr for MetadataStrategy {
  type Err = anyhow::Error;
  fn from_str(s: &str) -> Result<Self, Self::Err> {
    match s {
      "instance_metadata" => Ok(Self::InstanceMetadata),
      "testing" => Ok(Self::Testing),
      _ => Err(anyhow::anyhow!("Invalid metadata strategy '{}'", s)),
    }
  }
}

#[derive(Clone)]
pub struct Client(Arc<ClientInner>);

impl Client {
  pub fn new(metadata_strategy: MetadataStrategy) -> Self {
    let http_without_compression = reqwest::ClientBuilder::new()
      .user_agent(crate::util::USER_AGENT)
      .connect_timeout(HTTP_CONNECT_TIMEOUT)
      .no_gzip()
      .no_deflate()
      .no_brotli()
      .build()
      .unwrap();
    Self(Arc::new(ClientInner {
      http_without_compression,
      access_token: Mutex::new(None),
      identity_tokens: Mutex::new(HashMap::new()),
      metadata_strategy,
    }))
  }
}

impl std::ops::Deref for Client {
  type Target = ClientInner;

  fn deref(&self) -> &Self::Target {
    &self.0
  }
}

#[allow(dead_code)]
pub struct ClientInner {
  http_without_compression: reqwest::Client,
  metadata_strategy: MetadataStrategy,
  access_token: Mutex<Option<(String, Instant)>>,
  /// OIDC identity tokens by audience, with the instant they are treated as
  /// expired.
  identity_tokens: Mutex<HashMap<String, (String, Instant)>>,
}

/// How long an identity token is reused. The metadata server mints them with
/// a one hour lifetime; refreshing well before that keeps clock skew and a
/// long request from presenting an expired token.
const IDENTITY_TOKEN_TTL: Duration = Duration::from_secs(45 * 60);

#[allow(dead_code)]
impl ClientInner {
  pub fn http(&self) -> &'static reqwest::Client {
    crate::util::shared_http_client()
  }

  pub fn http_without_compression(&self) -> &reqwest::Client {
    &self.http_without_compression
  }

  pub async fn get_access_token(&self) -> Result<String, anyhow::Error> {
    match self.metadata_strategy {
      MetadataStrategy::InstanceMetadata => {
        {
          let mut guard = self.access_token.lock().unwrap();
          if let Some((token, expires_at)) = guard.clone() {
            // If the is still valid (doesnt expire within next 5 seconds, or is
            // already expired).
            if expires_at.checked_sub(Duration::from_secs(5)).unwrap()
              > Instant::now()
            {
              return Ok(token);
            }
            *guard = None;
          };
        }
        let url = "http://metadata.google.internal/computeMetadata/v1/instance/service-accounts/default/token";
        let resp = self
          .http()
          .get(url)
          .header("Metadata-Flavor", "Google")
          .send()
          .await?;
        if resp.status() != StatusCode::OK {
          let status = resp.status();
          let text = resp.text().await?;
          return Err(anyhow::anyhow!(
            "failed to get access token from metadata server: status={} text='{}'",
            status,
            text
          ));
        }
        let token: AccessTokenResponse = resp.json().await?;
        let mut guard = self.access_token.lock().unwrap();
        let expires_at = Instant::now() + Duration::from_secs(token.expires_in);
        *guard = Some((token.access_token.clone(), expires_at));
        Ok(token.access_token)
      }
      MetadataStrategy::Testing => Ok("testing.access.token".to_owned()),
    }
  }

  /// An OIDC identity token for `audience`, minted for this instance's service
  /// account. This is what a Cloud Run service that only admits authenticated
  /// invokers checks. `None` under the testing strategy, where nothing checks
  /// it.
  pub async fn get_identity_token(
    &self,
    audience: &str,
  ) -> Result<Option<String>, anyhow::Error> {
    match self.metadata_strategy {
      MetadataStrategy::InstanceMetadata => {
        {
          let guard = self.identity_tokens.lock().unwrap();
          if let Some((token, expires_at)) = guard.get(audience)
            && *expires_at > Instant::now()
          {
            return Ok(Some(token.clone()));
          }
        }
        let url = "http://metadata.google.internal/computeMetadata/v1/instance/service-accounts/default/identity";
        let resp = self
          .http()
          .get(url)
          .query(&[("audience", audience)])
          .header("Metadata-Flavor", "Google")
          .send()
          .await?;
        if resp.status() != StatusCode::OK {
          let status = resp.status();
          let text = resp.text().await?;
          return Err(anyhow::anyhow!(
            "failed to get identity token from metadata server: status={} text='{}'",
            status,
            text
          ));
        }
        let token = resp.text().await?;
        self.identity_tokens.lock().unwrap().insert(
          audience.to_owned(),
          (token.clone(), Instant::now() + IDENTITY_TOKEN_TTL),
        );
        Ok(Some(token))
      }
      MetadataStrategy::Testing => Ok(None),
    }
  }
}

#[derive(Clone)]
pub struct Queue {
  pub(crate) client: Client,
  pub(crate) id: String,
  pub(crate) endpoint: String,
}

impl Queue {
  pub fn new(client: Client, id: String, endpoint: Option<String>) -> Self {
    Self {
      client,
      id,
      endpoint: endpoint
        .unwrap_or_else(|| "https://cloudtasks.googleapis.com/".into()),
    }
  }

  #[instrument("gcp::Queue::task_buffer", skip(self), err, fields(queue_id = self.id
  ))]
  pub async fn task_buffer(
    &self,
    id: Option<String>,
    body: Option<Bytes>,
  ) -> Result<(), anyhow::Error> {
    let task_id = if let Some(id) = id {
      format!("/{}", id)
    } else {
      "".to_owned()
    };
    let url = format!(
      "{}/v2beta3/{}/tasks{}:buffer",
      self.endpoint, self.id, task_id
    );
    let token = self.client.get_access_token().await?;
    let req = self.client.http().post(url).bearer_auth(token);
    let req = if let Some(body) = body {
      req.body(body)
    } else {
      req
    };
    let resp = req.send().await?;
    let status = resp.status();
    if status != StatusCode::OK {
      let body = resp.text().await?;
      return Err(anyhow::anyhow!(
        "Failed to create task (status={status}): {body}"
      ));
    }
    Ok(())
  }
}
