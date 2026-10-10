// Copyright 2024 the JSR authors. All rights reserved. MIT license.
use anyhow::Context;
use bytes::Bytes;
use reqwest::StatusCode;
use serde::Deserialize;
use serde::Serialize;
use std::str::FromStr;
use std::sync::Arc;
use std::sync::Mutex;
use std::time::Duration;
use std::time::Instant;
use std::time::SystemTime;
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
  /// Sign access token requests with a service account key, for where there is
  /// no metadata server (the Cloudflare Worker).
  ServiceAccountKey,
  /// Returned fixed fake tokens for testing.
  Testing,
}

impl FromStr for MetadataStrategy {
  type Err = anyhow::Error;
  fn from_str(s: &str) -> Result<Self, Self::Err> {
    match s {
      "instance_metadata" => Ok(Self::InstanceMetadata),
      "service_account_key" => Ok(Self::ServiceAccountKey),
      "testing" => Ok(Self::Testing),
      _ => Err(anyhow::anyhow!("Invalid metadata strategy '{}'", s)),
    }
  }
}

#[derive(Deserialize)]
struct ServiceAccountKey {
  client_email: String,
  private_key: String,
  token_uri: String,
}

#[derive(Serialize)]
struct ServiceAccountClaims<'a> {
  iss: &'a str,
  scope: &'a str,
  aud: &'a str,
  iat: u64,
  exp: u64,
}

#[derive(Clone)]
pub struct Client(Arc<ClientInner>);

impl Client {
  pub fn new(
    metadata_strategy: MetadataStrategy,
    service_account_key: Option<&str>,
  ) -> Self {
    let service_account_key = match metadata_strategy {
      MetadataStrategy::ServiceAccountKey => Some(
        serde_json::from_str(
          service_account_key.expect("GCP_SERVICE_ACCOUNT_KEY must be set"),
        )
        .expect("GCP_SERVICE_ACCOUNT_KEY is not a service account key"),
      ),
      _ => None,
    };
    let http_without_compression = crate::util::http_client_builder()
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
      metadata_strategy,
      service_account_key,
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
  service_account_key: Option<ServiceAccountKey>,
  access_token: Mutex<Option<(String, Instant)>>,
}

#[allow(dead_code)]
impl ClientInner {
  pub fn http(&self) -> &'static reqwest::Client {
    crate::util::shared_http_client()
  }

  pub fn http_without_compression(&self) -> &reqwest::Client {
    &self.http_without_compression
  }

  pub async fn get_access_token(&self) -> Result<String, anyhow::Error> {
    if self.metadata_strategy == MetadataStrategy::Testing {
      return Ok("testing.access.token".to_owned());
    }
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
    let token = match self.metadata_strategy {
      MetadataStrategy::InstanceMetadata => {
        self.instance_metadata_access_token().await?
      }
      MetadataStrategy::ServiceAccountKey => {
        self.service_account_access_token().await?
      }
      MetadataStrategy::Testing => unreachable!(),
    };
    let mut guard = self.access_token.lock().unwrap();
    let expires_at = Instant::now() + Duration::from_secs(token.expires_in);
    *guard = Some((token.access_token.clone(), expires_at));
    Ok(token.access_token)
  }

  async fn instance_metadata_access_token(
    &self,
  ) -> Result<AccessTokenResponse, anyhow::Error> {
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
    Ok(resp.json().await?)
  }

  async fn service_account_access_token(
    &self,
  ) -> Result<AccessTokenResponse, anyhow::Error> {
    let key = self
      .service_account_key
      .as_ref()
      .context("no service account key")?;
    let now = SystemTime::now()
      .duration_since(SystemTime::UNIX_EPOCH)?
      .as_secs();
    let assertion = jsonwebtoken::encode(
      &jsonwebtoken::Header::new(jsonwebtoken::Algorithm::RS256),
      &ServiceAccountClaims {
        iss: &key.client_email,
        scope: "https://www.googleapis.com/auth/cloud-platform",
        aud: &key.token_uri,
        iat: now,
        exp: now + 3600,
      },
      &jsonwebtoken::EncodingKey::from_rsa_pem(key.private_key.as_bytes())?,
    )?;
    let resp = self
      .http()
      .post(&key.token_uri)
      .form(&[
        ("grant_type", "urn:ietf:params:oauth:grant-type:jwt-bearer"),
        ("assertion", &assertion),
      ])
      .send()
      .await?;
    if resp.status() != StatusCode::OK {
      let status = resp.status();
      let text = resp.text().await?;
      return Err(anyhow::anyhow!(
        "failed to get access token for service account: status={} text='{}'",
        status,
        text
      ));
    }
    Ok(resp.json().await?)
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
