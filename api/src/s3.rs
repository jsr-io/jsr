// Copyright 2024 the JSR authors. All rights reserved. MIT license.
use std::pin::Pin;
use std::sync::Arc;

use crate::task_queue::DynamicBackgroundTaskQueue;
use crate::task_queue::RestartableTask;
use crate::task_queue::RestartableTaskResult;
use bytes::Bytes;
use futures::Future;
use futures::FutureExt;
use futures::Stream;
use futures::StreamExt;
use futures::TryStreamExt;
use futures::join;
use hyper::StatusCode;
use s3::serde_types::ListBucketResult;
use thiserror::Error;
use tracing::instrument;

#[derive(Debug, Error, deno_error::JsError)]
#[class(generic)]
pub enum S3Error {
  #[error("request to S3 timed out")]
  RequestTimeout,
  #[error("too many requests to S3")]
  TooManyRequests,
  #[error("server error when communicating with S3: {0} ({0:?})")]
  Server(StatusCode),
  #[error("client error when communicating with S3: {0} ({0:?})")]
  Client(StatusCode),
  #[error(transparent)]
  S3(#[from] s3::error::S3Error),
  #[error("stream failed: {0}")]
  Stream(anyhow::Error),
}

impl S3Error {
  /// 408, 429, and 5xx errors are retryable.
  /// https://cloud.google.com/storage/docs/retry-strategy
  pub fn is_retryable(&self) -> bool {
    matches!(
      self,
      Self::RequestTimeout | Self::TooManyRequests | Self::Server(_)
    )
  }
}

#[derive(Clone)]
pub struct Bucket {
  pub(crate) bucket: Box<s3::Bucket>,
  pub(crate) name: String,
}

impl Bucket {
  pub fn new(
    name: String,
    region: s3::Region,
    credentials: s3::creds::Credentials,
  ) -> Result<Self, S3Error> {
    let bucket = s3::Bucket::new(&name, region, credentials)?.with_path_style();

    Ok(Self { bucket, name })
  }

  fn check_status(status_code: u16) -> Result<(), S3Error> {
    if status_code == StatusCode::REQUEST_TIMEOUT {
      return Err(S3Error::RequestTimeout);
    }
    if status_code == StatusCode::TOO_MANY_REQUESTS {
      return Err(S3Error::TooManyRequests);
    }
    if (500..600).contains(&status_code) {
      return Err(S3Error::Server(StatusCode::from_u16(status_code).unwrap()));
    }
    if (400..500).contains(&status_code) {
      return Err(S3Error::Client(StatusCode::from_u16(status_code).unwrap()));
    }
    Ok(())
  }

  #[cfg(test)]
  pub async fn create(
    name: String,
    region: s3::Region,
    credentials: s3::creds::Credentials,
  ) -> Result<Self, S3Error> {
    let bucket = s3::Bucket::create_with_path_style(
      &name,
      region,
      credentials,
      s3::BucketConfiguration::private(),
    )
    .await?;

    Ok(Self {
      bucket: bucket.bucket,
      name,
    })
  }

  #[instrument(name = "s3::Bucket::download", skip(self), err, fields(bucket = %self.name))]
  pub async fn download(&self, path: &str) -> Result<Option<Bytes>, S3Error> {
    let resp = self.bucket.get_object(path).await?;

    if resp.status_code() == 404 {
      return Ok(None);
    }

    Bucket::check_status(resp.status_code())?;
    Ok(Some(resp.into_bytes()))
  }

  #[instrument(name = "s3::Bucket::download_stream", skip(self), err, fields(bucket = %self.name))]
  pub async fn download_stream(
    &self,
    path: &str,
    offset: Option<usize>,
  ) -> Result<Option<impl Stream<Item = Result<Bytes, S3Error>> + use<>>, S3Error>
  {
    if let Some(offset) = offset {
      let resp = self
        .bucket
        .get_object_range(path, offset as _, None)
        .await?;
      if resp.status_code() == 404 || resp.status_code() == 416 {
        return Ok(None);
      }

      Ok(Some(
        futures::stream::once(async { Ok(resp.into_bytes()) }).boxed(),
      ))
    } else {
      let resp = self.bucket.get_object_stream(path).await?;
      if resp.status_code == 404 || resp.status_code == 416 {
        return Ok(None);
      }

      Ok(Some(resp.bytes.map(|e| e.map_err(S3Error::S3)).boxed()))
    }
  }

  #[instrument(name = "s3::Bucket::upload", skip(self, data), err, fields(bucket = %self.name, size = %data.len()))]
  pub async fn upload(
    &self,
    path: &str,
    data: Bytes,
    options: &crate::gcp::GcsUploadOptions<'_>,
  ) -> Result<(), S3Error> {
    let mut builder = self
      .bucket
      .put_object_builder(path, data.as_ref())
      .with_content_encoding(if options.gzip_encoded {
        "gzip"
      } else {
        "identity"
      })?;

    if let Some(content_type) = &options.content_type {
      builder = builder.with_content_type(content_type);
    }
    if let Some(cache_control) = &options.cache_control {
      builder = builder.with_cache_control(cache_control)?;
    }

    let resp = builder.execute().await?;
    Bucket::check_status(resp.status_code())?;

    Ok(())
  }

  #[instrument(
    name = "s3::Bucket::upload_stream",
    skip(self, stream),
    err,
    fields(bucket = %self.name)
  )]
  pub async fn upload_stream(
    &self,
    path: &str,
    stream: &mut (impl tokio::io::AsyncRead + Unpin + Send),
    options: &crate::gcp::GcsUploadOptions<'_>,
  ) -> Result<(), S3Error> {
    let mut builder = self
      .bucket
      .put_object_stream_builder(path)
      .with_content_encoding(if options.gzip_encoded {
        "gzip"
      } else {
        "identity"
      })?;

    if let Some(content_type) = &options.content_type {
      builder = builder.with_content_type(content_type);
    }
    if let Some(cache_control) = &options.cache_control {
      builder = builder.with_cache_control(cache_control)?;
    }

    let resp = builder.execute_stream(stream).await?;
    Bucket::check_status(resp.status_code())?;

    Ok(())
  }

  #[instrument(name = "s3::Bucket::list", skip(self), err, fields(bucket = %self.name))]
  pub async fn list(
    &self,
    path: &str,
  ) -> Result<Vec<ListBucketResult>, S3Error> {
    let list = self.bucket.list(path.to_string(), None).await?;
    Ok(list)
  }

  #[instrument(name = "s3::Bucket::delete", skip(self), err, fields(bucket = %self.name))]
  pub async fn delete_file(&self, path: &str) -> Result<bool, S3Error> {
    let resp = self.bucket.delete_object(path).await?;

    if resp.status_code() == 404 {
      return Ok(true);
    }
    Bucket::check_status(resp.status_code())?;
    Ok(false)
  }
}

#[allow(dead_code)]
#[derive(Clone)]
pub struct BucketWithQueue {
  pub bucket: Bucket,
  upload_queue: DynamicBackgroundTaskQueue<UploadTask>,
  download_queue: DynamicBackgroundTaskQueue<DownloadTask>,
  delete_queue: DynamicBackgroundTaskQueue<DeleteFileTask>,
  list_queue: DynamicBackgroundTaskQueue<ListDirectoryTask>,
}

impl BucketWithQueue {
  pub fn new(bucket: Bucket) -> Self {
    Self {
      bucket,
      upload_queue: DynamicBackgroundTaskQueue::default(),
      download_queue: DynamicBackgroundTaskQueue::default(),
      delete_queue: DynamicBackgroundTaskQueue::default(),
      list_queue: DynamicBackgroundTaskQueue::default(),
    }
  }

  #[instrument(
    name = "BucketWithQueue::upload",
    skip(self, body, options),
    err
  )]
  pub async fn upload(
    &self,
    path: Arc<str>,
    body: UploadTaskBody,
    options: crate::gcp::GcsUploadOptions<'static>,
  ) -> Result<(), S3Error> {
    self
      .upload_queue
      .run(UploadTask {
        bucket: self.bucket.clone(),
        path,
        body,
        options,
      })
      .await
      .unwrap()
  }

  #[allow(dead_code)]
  #[instrument(name = "BucketWithQueue::download", skip(self), err)]
  pub async fn download(
    &self,
    path: Arc<str>,
  ) -> Result<Option<Bytes>, S3Error> {
    self
      .download_queue
      .run(DownloadTask {
        bucket: self.bucket.clone(),
        path,
      })
      .await
      .unwrap()
  }

  #[allow(dead_code)]
  #[instrument(name = "BucketWithQueue::delete_file", skip(self), err)]
  pub async fn delete_file(&self, path: Arc<str>) -> Result<bool, S3Error> {
    self
      .delete_queue
      .run(DeleteFileTask {
        bucket: self.bucket.clone(),
        path,
      })
      .await
      .unwrap()
  }

  #[allow(dead_code)]
  #[instrument(name = "BucketWithQueue::delete_directory", skip(self), err)]
  pub async fn delete_directory(&self, path: Arc<str>) -> Result<(), S3Error> {
    let list = self
      .list_queue
      .run(ListDirectoryTask {
        bucket: self.bucket.clone(),
        path,
      })
      .await
      .unwrap()?;

    if !list.is_empty() {
      let stream = futures::stream::iter(list)
        .map(|item| self.delete_file(item.name.into()))
        .buffer_unordered(64);

      let _ = stream.try_collect::<Vec<_>>().await?;
    }

    Ok(())
  }
}

struct UploadTask {
  bucket: Bucket,
  path: Arc<str>,
  body: UploadTaskBody,
  options: crate::gcp::GcsUploadOptions<'static>,
}

pub enum UploadTaskBody {
  Bytes(Bytes),
  Stream(
    Box<
      dyn Stream<Item = Result<Bytes, std::io::Error>> + Unpin + Send + 'static,
    >,
  ),
}

impl RestartableTask for UploadTask {
  type Ok = ();
  type Err = S3Error;
  type Fut =
    Pin<Box<dyn Future<Output = RestartableTaskResult<Self>> + Send + 'static>>;

  fn run(self) -> Self::Fut {
    async move {
      match self.body {
        UploadTaskBody::Bytes(data) => {
          let bytes = data.clone();
          let res = self.bucket.upload(&self.path, data, &self.options).await;
          match res {
            Ok(()) => RestartableTaskResult::Ok(()),
            Err(e) if e.is_retryable() => {
              RestartableTaskResult::Backoff(UploadTask {
                bucket: self.bucket,
                path: self.path,
                body: UploadTaskBody::Bytes(bytes),
                options: self.options,
              })
            }
            Err(e) => RestartableTaskResult::Error(e),
          }
        }
        UploadTaskBody::Stream(mut stream) => {
          // Create a duplex stream that buffers all chunks so that we can retry
          // failed uploads later if needed.
          let (mut reader, mut writer) = tokio::io::duplex(64 * 1024);
          let stream_fut = async move {
            use tokio::io::AsyncWriteExt;
            let mut retry_buffer = Vec::new();
            while let Some(res) = stream.next().await {
              let chunk = res?;
              retry_buffer.extend_from_slice(&chunk);
              writer.write_all(&chunk).await.map_err(|e| {
                anyhow::anyhow!("writer.write_all() failed: {e}")
              })?;
            }
            drop(writer);
            Ok::<_, anyhow::Error>(retry_buffer)
          };
          let upload_fut =
            self
              .bucket
              .upload_stream(&self.path, &mut reader, &self.options);
          let (stream_res, upload_res) = join!(stream_fut, upload_fut);
          match (stream_res, upload_res) {
            (Ok(_), Ok(())) => RestartableTaskResult::Ok(()),
            (Ok(retry_buffer), Err(e)) if e.is_retryable() => {
              RestartableTaskResult::Backoff(UploadTask {
                bucket: self.bucket,
                path: self.path,
                body: UploadTaskBody::Bytes(Bytes::from(retry_buffer)),
                options: self.options,
              })
            }
            (_, Err(e)) => RestartableTaskResult::Error(e),
            (Err(e), _) => RestartableTaskResult::Error(S3Error::Stream(e)),
          }
        }
      }
    }
    .boxed()
  }
}

struct DownloadTask {
  bucket: Bucket,
  path: Arc<str>,
}

impl RestartableTask for DownloadTask {
  type Ok = Option<Bytes>;
  type Err = S3Error;
  type Fut =
    Pin<Box<dyn Future<Output = RestartableTaskResult<Self>> + Send + 'static>>;

  fn run(self) -> Self::Fut {
    async move {
      let res = self.bucket.download(&self.path).await;
      match res {
        Ok(data) => RestartableTaskResult::Ok(data),
        Err(e) if e.is_retryable() => {
          RestartableTaskResult::Backoff(DownloadTask {
            bucket: self.bucket,
            path: self.path,
          })
        }
        Err(e) => RestartableTaskResult::Error(e),
      }
    }
    .boxed()
  }
}

struct DeleteFileTask {
  bucket: Bucket,
  path: Arc<str>,
}

impl RestartableTask for DeleteFileTask {
  type Ok = bool;
  type Err = S3Error;
  type Fut =
    Pin<Box<dyn Future<Output = RestartableTaskResult<Self>> + Send + 'static>>;

  fn run(self) -> Self::Fut {
    async move {
      let res = self.bucket.delete_file(&self.path).await;
      match res {
        Ok(data) => RestartableTaskResult::Ok(data),
        Err(e) if e.is_retryable() => {
          RestartableTaskResult::Backoff(DeleteFileTask {
            bucket: self.bucket,
            path: self.path,
          })
        }
        Err(e) => RestartableTaskResult::Error(e),
      }
    }
    .boxed()
  }
}

struct ListDirectoryTask {
  bucket: Bucket,
  path: Arc<str>,
}

impl RestartableTask for ListDirectoryTask {
  type Ok = Vec<ListBucketResult>;
  type Err = S3Error;
  type Fut =
    Pin<Box<dyn Future<Output = RestartableTaskResult<Self>> + Send + 'static>>;

  fn run(self) -> Self::Fut {
    async move {
      let res = self.bucket.list(&self.path).await;
      match res {
        Ok(data) => RestartableTaskResult::Ok(data),
        Err(e) if e.is_retryable() => {
          RestartableTaskResult::Backoff(ListDirectoryTask {
            bucket: self.bucket,
            path: self.path,
          })
        }
        Err(e) => RestartableTaskResult::Error(e),
      }
    }
    .boxed()
  }
}

/// https://github.com/minio/minio
#[cfg(test)]
pub struct FakeS3Tester {
  proc: Option<std::process::Child>,
  pub port: u16,
}

#[cfg(test)]
impl FakeS3Tester {
  pub async fn new() -> Self {
    use rand::Rng;
    let mut rng = rand::thread_rng();
    let port = rng.gen_range(20001..30001);
    //let port = PORT_PICKER.pick().await;
    let mut t = Self { port, proc: None };
    t.start();
    t
  }

  pub fn endpoint(&self) -> String {
    format!("http://localhost:{}", self.port)
  }

  pub async fn create_bucket(&self, bucket: &str) -> Bucket {
    Bucket::create(
      bucket.to_owned(),
      s3::Region::Custom {
        region: "us-east-1".to_string(),
        endpoint: self.endpoint().to_owned(),
      },
      s3::creds::Credentials {
        access_key: Some("minioadmin".to_string()),
        secret_key: Some("minioadmin".to_string()),
        security_token: None,
        session_token: None,
        expiration: None,
      },
    )
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
      "/../tools/bin/darwin-arm64/minio"
    );

    #[cfg(all(target_os = "macos", target_arch = "x86_64"))]
    let p = concat!(
      env!("CARGO_MANIFEST_DIR"),
      "/../tools/bin/darwin-amd64/minio"
    );

    #[cfg(target_os = "linux")]
    let p = concat!(
      env!("CARGO_MANIFEST_DIR"),
      "/../tools/bin/linux-amd64/minio"
    );

    let mut dir = std::env::temp_dir();
    dir.push("fake-s3-server");

    println!("starting fake s3 server: {}", p);
    let mut proc = std::process::Command::new(p)
      .arg("server")
      .arg(format!("--address=:{}", self.port))
      .arg("--quiet")
      .arg(dir)
      .process_group(0)
      .stderr(Stdio::piped())
      .spawn()
      .unwrap();

    // Wait for one line of output from stderr.
    let stderr = proc.stderr.take().unwrap();
    let mut stderr = BufReader::new(stderr);
    let mut first_line = String::new();
    stderr.read_line(&mut first_line).unwrap();
    if !first_line.contains("minioadmin:minioadmin") {
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
impl Drop for FakeS3Tester {
  fn drop(&mut self) {
    if let Some(proc) = self.proc.as_mut()
      && let Err(err) = proc.kill()
    {
      eprintln!("failed to kill FakeS3Tester on drop: {err}");
    }
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  #[tokio::test]
  async fn gcs_upload_download() {
    let tester = FakeS3Tester::new().await;
    let bucket = tester.create_bucket("testbucket").await;

    bucket
      .upload(
        "upload_download.txt",
        "hello world".as_bytes().to_vec().into(),
        &crate::gcp::GcsUploadOptions {
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
