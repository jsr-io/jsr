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

#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub enum MetadataStrategy {
  /// Get authentication information from the instance metadata server.
  InstanceMetadata,
  /// Mint access tokens from a GCP service account key (provided separately
  /// via `--gcp_service_account_key`). Used when running off-GCP, e.g. in a
  /// Cloudflare Container where the instance metadata server is unavailable.
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
struct ServiceAccountKeyFile {
  client_email: String,
  private_key: String,
}

/// A parsed GCP service account, with the RSA signing key pre-parsed so token
/// refreshes don't re-parse the PEM on every call.
struct ServiceAccount {
  client_email: String,
  encoding_key: jsonwebtoken::EncodingKey,
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
  pub fn new(
    metadata_strategy: MetadataStrategy,
    service_account_key: Option<String>,
  ) -> Result<Self, anyhow::Error> {
    let http_without_compression = reqwest::ClientBuilder::new()
      .user_agent(crate::util::USER_AGENT)
      .connect_timeout(HTTP_CONNECT_TIMEOUT)
      .no_gzip()
      .no_deflate()
      .no_brotli()
      .build()
      .unwrap();
    // Parse the service account key once at startup (rather than on every token
    // refresh), surfacing a malformed key as a clear boot-time error.
    let service_account = match metadata_strategy {
      MetadataStrategy::ServiceAccountKey => {
        let json = service_account_key.ok_or_else(|| {
          anyhow::anyhow!(
            "metadata_strategy is 'service_account_key' but no \
             --gcp_service_account_key was provided"
          )
        })?;
        let key: ServiceAccountKeyFile = serde_json::from_str(json.trim())
          .map_err(|e| {
            anyhow::anyhow!("failed to parse GCP service account key JSON: {e}")
          })?;
        let encoding_key =
          jsonwebtoken::EncodingKey::from_rsa_pem(key.private_key.as_bytes())
            .map_err(|e| {
            anyhow::anyhow!("failed to parse GCP service account RSA key: {e}")
          })?;
        Some(ServiceAccount {
          client_email: key.client_email,
          encoding_key,
        })
      }
      _ => None,
    };
    Ok(Self(Arc::new(ClientInner {
      http_without_compression,
      access_token: Mutex::new(None),
      metadata_strategy,
      service_account,
    })))
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
  /// Parsed service account, if using the ServiceAccountKey strategy.
  service_account: Option<ServiceAccount>,
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
      MetadataStrategy::ServiceAccountKey => {
        {
          let guard = self.access_token.lock().unwrap();
          if let Some((token, expires_at)) = guard.clone()
            && expires_at.checked_sub(Duration::from_secs(5)).unwrap()
              > Instant::now()
          {
            return Ok(token);
          }
        }
        let sa = self
          .service_account
          .as_ref()
          .expect("service_account must be set");
        let now = std::time::SystemTime::now()
          .duration_since(std::time::UNIX_EPOCH)
          .unwrap()
          .as_secs();
        let claims = JwtClaims {
          iss: sa.client_email.clone(),
          scope: "https://www.googleapis.com/auth/cloud-platform".to_owned(),
          aud: "https://oauth2.googleapis.com/token".to_owned(),
          iat: now,
          exp: now + 3600,
        };
        let header = jsonwebtoken::Header::new(jsonwebtoken::Algorithm::RS256);
        let jwt = jsonwebtoken::encode(&header, &claims, &sa.encoding_key)?;
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
