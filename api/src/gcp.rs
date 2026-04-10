// Copyright 2024 the JSR authors. All rights reserved. MIT license.
use bytes::Bytes;
use hyper::StatusCode;
use serde::Deserialize;
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

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MetadataStrategy {
  /// Get authentication information from the instance metadata server.
  InstanceMetadata,
  /// Use a GCP service account key JSON to mint access tokens.
  ServiceAccountKey(String),
  /// Returned fixed fake tokens for testing.
  Testing,
}

impl FromStr for MetadataStrategy {
  type Err = anyhow::Error;
  fn from_str(s: &str) -> Result<Self, Self::Err> {
    match s {
      "instance_metadata" => Ok(Self::InstanceMetadata),
      "testing" => Ok(Self::Testing),
      _ => {
        // Try to parse as a JSON service account key
        if s.starts_with('{') {
          Ok(Self::ServiceAccountKey(s.to_owned()))
        } else {
          Err(anyhow::anyhow!("Invalid metadata strategy '{}'", s))
        }
      }
    }
  }
}

#[derive(Deserialize)]
struct ServiceAccountKeyFile {
  client_email: String,
  private_key: String,
}

#[derive(serde::Serialize)]
struct JwtClaims {
  iss: String,
  scope: String,
  aud: String,
  iat: u64,
  exp: u64,
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
    let service_account_key = match &metadata_strategy {
      MetadataStrategy::ServiceAccountKey(json) => Some(
        serde_json::from_str::<ServiceAccountKeyFile>(json)
          .expect("Failed to parse GCP service account key JSON"),
      ),
      _ => None,
    };
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
  access_token: Mutex<Option<(String, Instant)>>,
  /// Parsed service account key, if using ServiceAccountKey strategy.
  service_account_key: Option<ServiceAccountKeyFile>,
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
    match &self.metadata_strategy {
      MetadataStrategy::InstanceMetadata => {
        {
          let mut guard = self.access_token.lock().unwrap();
          if let Some((token, expires_at)) = guard.clone() {
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
      MetadataStrategy::ServiceAccountKey(_) => {
        {
          let guard = self.access_token.lock().unwrap();
          if let Some((token, expires_at)) = guard.clone() {
            if expires_at.checked_sub(Duration::from_secs(5)).unwrap()
              > Instant::now()
            {
              return Ok(token);
            }
          }
        }
        let sa_key = self
          .service_account_key
          .as_ref()
          .expect("service_account_key must be set");
        let now = std::time::SystemTime::now()
          .duration_since(std::time::UNIX_EPOCH)
          .unwrap()
          .as_secs();
        let claims = JwtClaims {
          iss: sa_key.client_email.clone(),
          scope: "https://www.googleapis.com/auth/cloud-platform".to_owned(),
          aud: "https://oauth2.googleapis.com/token".to_owned(),
          iat: now,
          exp: now + 3600,
        };
        let header = jsonwebtoken::Header::new(jsonwebtoken::Algorithm::RS256);
        let encoding_key = jsonwebtoken::EncodingKey::from_rsa_pem(
          sa_key.private_key.as_bytes(),
        )?;
        let jwt = jsonwebtoken::encode(&header, &claims, &encoding_key)?;
        let resp = self
          .http()
          .post("https://oauth2.googleapis.com/token")
          .form(&[
            ("grant_type", "urn:ietf:params:oauth:grant-type:jwt-bearer"),
            ("assertion", &jwt),
          ])
          .send()
          .await?;
        if resp.status() != StatusCode::OK {
          let status = resp.status();
          let text = resp.text().await?;
          return Err(anyhow::anyhow!(
            "failed to exchange JWT for access token: status={} text='{}'",
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
