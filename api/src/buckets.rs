// Copyright 2024 the JSR authors. All rights reserved. MIT license.
use std::pin::Pin;
use std::sync::Arc;

use bytes::Bytes;
use futures::join;
use futures::Future;
use futures::FutureExt;
use futures::Stream;
use futures::StreamExt;
use tokio::sync::mpsc;
use tokio_stream::wrappers::UnboundedReceiverStream;
use tracing::instrument;

use crate::gcp;
use crate::gcp::GcsError;
use crate::gcp::GcsUploadOptions;
use crate::task_queue::DynamicBackgroundTaskQueue;
use crate::task_queue::RestartableTask;
use crate::task_queue::RestartableTaskResult;

#[derive(Clone)]
pub struct BucketWithQueue {
  pub bucket: gcp::Bucket,
  upload_queue: DynamicBackgroundTaskQueue<UploadTask>,
  download_queue: DynamicBackgroundTaskQueue<DownloadTask>,
}

impl BucketWithQueue {
  pub fn new(bucket: gcp::Bucket) -> Self {
    Self {
      bucket,
      upload_queue: DynamicBackgroundTaskQueue::default(),
      download_queue: DynamicBackgroundTaskQueue::default(),
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
    options: GcsUploadOptions<'static>,
  ) -> Result<(), GcsError> {
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

  #[instrument(name = "BucketWithQueue::download", skip(self), err)]
  pub async fn download(
    &self,
    path: Arc<str>,
  ) -> Result<Option<Bytes>, GcsError> {
    self
      .download_queue
      .run(DownloadTask {
        bucket: self.bucket.clone(),
        path,
      })
      .await
      .unwrap()
  }
}

#[derive(Clone)]
pub struct Buckets {
  pub publishing_bucket: BucketWithQueue,
  pub modules_bucket: BucketWithQueue,
  pub docs_bucket: BucketWithQueue,
  pub npm_bucket: BucketWithQueue,
}

struct UploadTask {
  bucket: gcp::Bucket,
  path: Arc<str>,
  body: UploadTaskBody,
  options: GcsUploadOptions<'static>,
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
  type Err = gcp::GcsError;
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
          // Create a new stream that buffers all chunks so that we can retry
          // failed uploads later if needed.
          let (sender, receiver) =
            mpsc::unbounded_channel::<Result<Bytes, std::io::Error>>();
          let stream_fut = async move {
            let mut retry_buffer = Vec::new();
            while let Some(res) = stream.next().await {
              let chunk = res?;
              retry_buffer.extend_from_slice(&chunk);
              sender
                .send(Ok(chunk))
                .map_err(|_| anyhow::anyhow!("sender.send() failed"))?;
            }
            Ok::<_, anyhow::Error>(retry_buffer)
          };
          let stream = UnboundedReceiverStream::new(receiver);
          let upload_fut =
            self.bucket.upload_stream(&self.path, stream, &self.options);
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
            (Err(e), _) => RestartableTaskResult::Error(GcsError::Stream(e)),
          }
        }
      }
    }
    .boxed()
  }
}

struct DownloadTask {
  bucket: gcp::Bucket,
  path: Arc<str>,
}

impl RestartableTask for DownloadTask {
  type Ok = Option<Bytes>;
  type Err = gcp::GcsError;
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
