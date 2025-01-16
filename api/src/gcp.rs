// Copyright 2024 the JSR authors. All rights reserved. MIT license.
use bytes::Bytes;
use futures::Stream;
use hyper::HeaderMap;
use hyper::StatusCode;
use percent_encoding::NON_ALPHANUMERIC;
use reqwest::multipart;
use reqwest::multipart::Part;
use reqwest::Body;
use reqwest::Response;
use serde::Deserialize;
use serde_json::json;
use std::borrow::Cow;
use std::str::FromStr;
use std::sync::Arc;
use std::sync::Mutex;
use std::time::Duration;
use std::time::Instant;
use thiserror::Error;
use tracing::error;
use tracing::instrument;

pub const CACHE_CONTROL_IMMUTABLE: &str = "public, max-age=31536000, immutable";
pub const CACHE_CONTROL_DO_NOT_CACHE: &str = "no-cache, no-store, max-age=0";
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
    let http = reqwest::ClientBuilder::new()
      .connect_timeout(HTTP_CONNECT_TIMEOUT)
      .build()
      .unwrap();
    let http_without_compression = reqwest::ClientBuilder::new()
      .connect_timeout(HTTP_CONNECT_TIMEOUT)
      .no_gzip()
      .no_deflate()
      .no_brotli()
      .build()
      .unwrap();
    Self(Arc::new(ClientInner {
      http,
      http_without_compression,
      access_token: Mutex::new(None),
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

pub struct ClientInner {
  http: reqwest::Client,
  http_without_compression: reqwest::Client,
  metadata_strategy: MetadataStrategy,
  access_token: Mutex<Option<(String, Instant)>>,
}

impl ClientInner {
  pub fn http(&self) -> &reqwest::Client {
    &self.http
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
}

#[derive(Clone)]
pub struct Bucket {
  pub(crate) client: Client,
  pub(crate) name: String,
  pub(crate) endpoint: String,
}

#[derive(Debug, Error)]
pub enum GcsError {
  #[error("request to GCS timed out")]
  RequestTimeout,
  #[error("too many requests to GCS")]
  TooManyRequests,
  #[error("server error when communicating with GCS: {0} ({0:?})")]
  Server(StatusCode),
  #[error("failed to get access token: {0}")]
  AccessToken(anyhow::Error),
  #[error(transparent)]
  Reqwest(#[from] reqwest::Error),
  #[error("stream failed: {0}")]
  Stream(anyhow::Error),
}

impl GcsError {
  /// 408, 429, and 5xx errors are retryable.
  /// https://cloud.google.com/storage/docs/retry-strategy
  pub fn is_retryable(&self) -> bool {
    matches!(
      self,
      Self::RequestTimeout | Self::TooManyRequests | Self::Server(_)
    )
  }
}

#[derive(Debug)]
pub struct GcsUploadOptions<'a> {
  pub content_type: Option<Cow<'a, str>>,
  pub cache_control: Option<Cow<'a, str>>,
  pub gzip_encoded: bool,
}

impl Bucket {
  pub fn new(client: Client, name: String, endpoint: Option<String>) -> Self {
    Self {
      client,
      name,
      endpoint: endpoint
        .unwrap_or_else(|| "https://storage.googleapis.com".to_owned()),
    }
  }

  fn error_if_failed(resp: Response) -> Result<Response, GcsError> {
    let status_code = resp.status();
    if status_code == StatusCode::REQUEST_TIMEOUT {
      return Err(GcsError::RequestTimeout);
    }
    if status_code == StatusCode::TOO_MANY_REQUESTS {
      return Err(GcsError::TooManyRequests);
    }
    if status_code.is_server_error() {
      return Err(GcsError::Server(status_code));
    }
    let resp = resp.error_for_status()?;
    Ok(resp)
  }

  #[cfg(test)]
  pub async fn create(
    client: Client,
    name: String,
    endpoint: Option<String>,
  ) -> Result<Self, GcsError> {
    let bucket = Bucket::new(client, name, endpoint);
    let url = format!("{}/storage/v1/b", bucket.endpoint);
    let token = bucket
      .client
      .get_access_token()
      .await
      .map_err(GcsError::AccessToken)?;
    let resp = bucket
      .client
      .http()
      .post(&url)
      .bearer_auth(token)
      .json(&json!({"name": bucket.name}))
      .send()
      .await?;
    Bucket::error_if_failed(resp)?;
    Ok(bucket)
  }

  #[instrument(name = "gcp::Bucket::download_resp", skip(self), err, fields(bucket = %self.name))]
  pub async fn download_resp(&self, path: &str) -> Result<Response, GcsError> {
    let path = percent_encoding::utf8_percent_encode(path, NON_ALPHANUMERIC);
    let url = format!(
      "{}/storage/v1/b/{}/o/{}?alt=media",
      self.endpoint, self.name, path
    );
    let token = self
      .client
      .get_access_token()
      .await
      .map_err(GcsError::AccessToken)?;
    let resp = self
      .client
      .http()
      .get(url)
      .bearer_auth(token)
      .send()
      .await?;
    Ok(resp)
  }

  #[instrument(name = "gcp::Bucket::download", skip(self), err, fields(bucket = %self.name))]
  pub async fn download(&self, path: &str) -> Result<Option<Bytes>, GcsError> {
    let resp = self.download_resp(path).await?;
    if resp.status() == 404 {
      return Ok(None);
    }
    let resp = Bucket::error_if_failed(resp)?;
    let bytes = resp.bytes().await?;
    Ok(Some(bytes))
  }

  #[instrument(name = "gcp::Bucket::download_stream", skip(self), err, fields(bucket = %self.name))]
  pub async fn download_stream(
    &self,
    path: &str,
    offset: Option<usize>,
  ) -> Result<Option<impl Stream<Item = Result<Bytes, reqwest::Error>>>, GcsError>
  {
    self
      .download_stream_with_encoding(path, offset, "")
      .await
      .map(|x| x.map(|x| x.1))
  }

  #[instrument(name = "gcp::Bucket::download_stream_with_encoding", skip(self), err, fields(bucket = %self.name))]
  pub async fn download_stream_with_encoding(
    &self,
    path: &str,
    offset: Option<usize>,
    accept_encoding: &str,
  ) -> Result<
    Option<(HeaderMap, impl Stream<Item = Result<Bytes, reqwest::Error>>)>,
    GcsError,
  > {
    let path = percent_encoding::utf8_percent_encode(path, NON_ALPHANUMERIC);
    let url = format!(
      "{}/storage/v1/b/{}/o/{}?alt=media",
      self.endpoint, self.name, path
    );
    let token = self
      .client
      .get_access_token()
      .await
      .map_err(GcsError::AccessToken)?;
    let mut req = if !accept_encoding.is_empty() {
      self
        .client
        .http_without_compression()
        .get(url)
        .header("Accept-Encoding", accept_encoding)
    } else {
      self.client.http().get(url)
    };
    req = req.bearer_auth(token);
    if let Some(offset) = offset {
      // for syntax, refer to https://cloud.google.com/storage/docs/json_api/v1/parameters#offset
      req = req.header("Range", format!("bytes={offset}-"))
    }
    let resp = req.send().await?;
    if resp.status() == 404 || resp.status() == 416 {
      return Ok(None);
    }
    let mut resp = Bucket::error_if_failed(resp)?;
    let headers = std::mem::take(resp.headers_mut());
    Ok(Some((headers, resp.bytes_stream())))
  }

  async fn upload_inner(
    &self,
    path: &str,
    mut media_part: Part,
    options: &GcsUploadOptions<'_>,
  ) -> Result<(), GcsError> {
    let url = format!(
      "{}/upload/storage/v1/b/{}/o?uploadType=multipart",
      self.endpoint, self.name
    );
    let token = self
      .client
      .get_access_token()
      .await
      .map_err(GcsError::AccessToken)?;
    let request_builder = self.client.http().post(url).bearer_auth(token);
    let json = json!({
      "name": path,
      "cacheControl": options.cache_control,
      "contentEncoding": if options.gzip_encoded { "gzip" } else { "identity" },
    });
    if let Some(content_type) = &options.content_type {
      media_part = media_part.mime_str(content_type).unwrap();
    }
    let meta = Part::text(json.to_string())
      .mime_str("application/json")
      .unwrap();
    let form = multipart::Form::new()
      .part("meta", meta)
      .part("media", media_part);

    let resp = request_builder.multipart(form).send().await?;
    Bucket::error_if_failed(resp)?;

    Ok(())
  }

  #[instrument(name = "gcp::Bucket::upload", skip(self, data), err, fields(bucket = %self.name, size = %data.len()))]
  pub async fn upload(
    &self,
    path: &str,
    data: Bytes,
    options: &GcsUploadOptions<'_>,
  ) -> Result<(), GcsError> {
    self
      .upload_inner(path, Part::bytes(Cow::Owned(data.to_vec())), options)
      .await
  }

  #[instrument(name = "gcp::Bucket::upload_stream", skip(self, stream), err, fields(bucket = %self.name))]
  pub async fn upload_stream<
    S: Stream<Item = Result<Bytes, std::io::Error>> + Send + Sync + 'static,
  >(
    &self,
    path: &str,
    stream: S,
    options: &GcsUploadOptions<'_>,
  ) -> Result<(), GcsError> {
    self
      .upload_inner(path, Part::stream(Body::wrap_stream(stream)), options)
      .await
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

  #[instrument("gcp::Queue::task_buffer", skip(self), err, fields(queue_id = self.id))]
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

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BigQueryJobReference {
  pub job_id: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BigQueryQueryResult {
  pub job_reference: BigQueryJobReference,
  pub page_token: Option<String>,
  #[serde(default)]
  pub rows: Vec<serde_json::Value>,
  #[serde(default)]
  pub errors: Option<serde_json::Value>,
  pub job_complete: bool,
}

pub struct BigQuery {
  pub(crate) client: Client,
  pub(crate) project: String,
  pub(crate) endpoint: String,
}

impl BigQuery {
  pub fn new(
    client: Client,
    project: String,
    endpoint: Option<String>,
  ) -> Self {
    Self {
      client,
      project,
      endpoint: endpoint
        .unwrap_or_else(|| "https://bigquery.googleapis.com/".into()),
    }
  }

  #[instrument("gcp::BigQuery::query", skip(self), err, fields(project = %self.project))]
  pub async fn query(
    &self,
    query: &str,
    params: &[serde_json::Value],
  ) -> Result<BigQueryQueryResult, anyhow::Error> {
    let url = format!(
      "{}/bigquery/v2/projects/{}/queries",
      self.endpoint, self.project
    );
    let token = self.client.get_access_token().await?;
    let resp = self
      .client
      .http()
      .post(url)
      .bearer_auth(token)
      .json(&json!({ "query": query, "useLegacySql": false, "parameterMode": "NAMED", "queryParameters": params, "formatOptions": { "useInt64Timestamp": true } }))
      .send()
      .await?;
    let status = resp.status();
    if status != StatusCode::OK {
      let body = resp.text().await?;
      return Err(anyhow::anyhow!(
        "Failed to query BigQuery (status={status}): {body}"
      ));
    }
    let json = resp.json().await?;
    Ok(json)
  }

  #[instrument("gcp::BigQuery::get_query_results", skip(self), err, fields(project = %self.project))]
  pub async fn get_query_results(
    &self,
    job_id: &str,
    page_token: &str,
  ) -> Result<BigQueryQueryResult, anyhow::Error> {
    let url = format!(
      "{}/bigquery/v2/projects/{}/queries/{}?pageToken={}&formatOptions.useInt64Timestamp=true",
      self.endpoint, self.project, job_id, page_token
    );
    let token = self.client.get_access_token().await?;
    let resp = self
      .client
      .http()
      .get(url)
      .bearer_auth(token)
      .send()
      .await?;
    let status = resp.status();
    if status != StatusCode::OK {
      let body = resp.text().await?;
      return Err(anyhow::anyhow!(
        "Failed to get query results (status={status}): {body}"
      ));
    }
    let json = resp.json().await?;
    Ok(json)
  }
}

/// Fake Google Cloud Storage
/// https://github.com/fsouza/fake-gcs-server
#[cfg(test)]
pub struct FakeGcsTester {
  proc: Option<std::process::Child>,
  pub port: u16,
}

#[cfg(test)]
impl FakeGcsTester {
  pub async fn new() -> Self {
    use rand::Rng;
    let mut rng = rand::thread_rng();
    let port = rng.gen_range(10000..20001);
    //let port = PORT_PICKER.pick().await;
    let mut t = Self { port, proc: None };
    t.start();
    t
  }

  pub fn endpoint(&self) -> String {
    format!("http://localhost:{}", self.port)
  }

  pub async fn create_bucket(&self, bucket: &str) -> Bucket {
    let client = Client::new(MetadataStrategy::Testing);
    Bucket::create(client, bucket.to_owned(), Some(self.endpoint()))
      .await
      .unwrap()
  }

  fn start(&mut self) {
    use std::io::BufRead;
    use std::io::BufReader;
    use std::os::unix::process::CommandExt;
    use std::process::Stdio;

    assert!(self.proc.is_none());

    #[cfg(all(target_os = "macos", target_arch = "aarch64"))]
    let p = concat!(
      env!("CARGO_MANIFEST_DIR"),
      "/../tools/bin/darwin-arm64/fake-gcs-server"
    );

    #[cfg(all(target_os = "macos", target_arch = "x86_64"))]
    let p = concat!(
      env!("CARGO_MANIFEST_DIR"),
      "/../tools/bin/darwin-amd64/fake-gcs-server"
    );

    #[cfg(target_os = "linux")]
    let p = concat!(
      env!("CARGO_MANIFEST_DIR"),
      "/../tools/bin/linux-amd64/fake-gcs-server"
    );

    println!("starting fake gcs server: {}", p);
    let mut proc = std::process::Command::new(p)
      .arg(format!("-port={}", self.port))
      .arg("-scheme=http")
      .arg("-backend=memory")
      .process_group(0)
      .stderr(Stdio::piped())
      .spawn()
      .unwrap();

    // Wait for one line of output from stderr.
    let stderr = proc.stderr.take().unwrap();
    let mut stderr = BufReader::new(stderr);
    let mut first_line = String::new();
    stderr.read_line(&mut first_line).unwrap();
    if !first_line.contains("server started at http://") {
      panic!("failed to start fake gcs server: {first_line}");
    }

    // Then copy the rest of stderr to a sink to prevent fake-gcs-server from
    // blocking.
    std::thread::spawn(move || {
      std::io::copy(&mut stderr.into_inner(), &mut std::io::sink()).ok();
    });

    self.proc = Some(proc);
  }
}

#[cfg(test)]
impl Drop for FakeGcsTester {
  fn drop(&mut self) {
    if let Some(proc) = self.proc.as_mut() {
      if let Err(err) = proc.kill() {
        eprintln!("failed to kill FakeGcsTester on drop: {err}");
      }
    }
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  #[tokio::test]
  async fn gcs_upload_download() {
    let tester = FakeGcsTester::new().await;
    let bucket = tester.create_bucket("testbucket").await;

    bucket
      .upload(
        "upload_download.txt",
        "hello world".as_bytes().to_vec().into(),
        &GcsUploadOptions {
          content_type: None,
          cache_control: None,
          gzip_encoded: false,
        },
      )
      .await
      .unwrap();

    let response = bucket.download("upload_download.txt").await.unwrap();
    assert!(response.is_some());
    assert_eq!(response.unwrap().len(), 11);

    let response = bucket.download("does_not_exist.txt").await.unwrap();
    assert!(response.is_none());
  }
}
