// Copyright 2024 the JSR authors. All rights reserved. MIT license.

//! In-process cache for immutable objects in the modules bucket.
//!
//! Every path cached here already encodes a specific published version, so the
//! bytes behind it never change: a new publish writes a new path. That makes
//! the S3 path a safe cache key with no invalidation story to get wrong. The
//! package-level `meta.json` is *not* eligible — it is rewritten on every
//! publish — and neither is anything else addressed without a version.
//!
//! It exists because these reads were the largest source of repeated R2 round
//! trips. `<version>_meta.json` is fetched once per source-file view, to turn
//! imports into links, but is shared by every file in the version: in a
//! one-hour production sample 76% of the fetches were re-reads of an object
//! already pulled that same hour, and the busiest package fetched the identical
//! object 202 times.
//!
//! Misses are single-flighted, so concurrent requests for the same cold path
//! wait on one download instead of each issuing their own. Absent objects are
//! deliberately *not* cached: a 404 leaves the loader as an error so moka
//! discards it, which stops an object still being written during publish from
//! being remembered as missing.

use crate::s3::BucketWithQueue;
use crate::s3::S3Error;
use bytes::Bytes;
use std::sync::Arc;
use std::time::Duration;

/// Total bytes held across all entries.
///
/// Deliberately small. The objects here are version manifests and READMEs, so
/// this still holds many hundreds of them, and it has to share the API
/// container's memory limit with the doc-node cache. Both are now bounded in
/// bytes, where the doc-node cache used to be bounded only in entry count, so
/// the two together have a smaller worst case than that one cache did alone —
/// this needs no increase to the container's memory limit.
const MAX_TOTAL_BYTES: u64 = 32 * 1024 * 1024;

/// Objects larger than this are served but not retained. Without a ceiling one
/// outsized object could evict most of the working set to store a single entry.
const MAX_ENTRY_BYTES: usize = 2 * 1024 * 1024;

/// Drop entries that go unread for this long, so a burst of traffic across many
/// packages does not pin memory for the rest of the process's life.
const TIME_TO_IDLE: Duration = Duration::from_secs(30 * 60);

#[derive(Debug, thiserror::Error)]
enum LoadError {
  #[error(transparent)]
  S3(S3Error),
  /// Never surfaces to a caller: it is how a 404 escapes `try_get_with`
  /// without being cached.
  #[error("object not found")]
  Absent,
}

#[derive(Debug, thiserror::Error)]
#[error(transparent)]
pub struct ObjectCacheError(Arc<LoadError>);

#[derive(Clone)]
pub struct ObjectCache {
  cache: moka::future::Cache<Arc<str>, Bytes>,
}

impl Default for ObjectCache {
  fn default() -> Self {
    Self::new()
  }
}

impl ObjectCache {
  pub fn new() -> Self {
    Self {
      cache: moka::future::Cache::builder()
        .max_capacity(MAX_TOTAL_BYTES)
        .weigher(|_path, bytes: &Bytes| {
          bytes.len().try_into().unwrap_or(u32::MAX)
        })
        .time_to_idle(TIME_TO_IDLE)
        .build(),
    }
  }

  /// Download `path` from `bucket`, serving it from memory when it has been
  /// read before. `Ok(None)` means the object does not exist.
  ///
  /// Only call this for paths whose contents are immutable — see the module
  /// docs. A mutable path (the package-level `meta.json`, an npm packument)
  /// would be served stale until it fell out of the cache.
  pub async fn download(
    &self,
    bucket: &BucketWithQueue,
    path: Arc<str>,
  ) -> Result<Option<Bytes>, ObjectCacheError> {
    let loaded = self
      .cache
      .try_get_with(path.clone(), async {
        match bucket.download(path.clone()).await {
          Ok(Some(bytes)) => Ok(bytes),
          Ok(None) => Err(LoadError::Absent),
          Err(err) => Err(LoadError::S3(err)),
        }
      })
      .await;

    match loaded {
      Ok(bytes) => {
        // The fetcher, and anyone who joined its single flight, still gets the
        // bytes; they just are not kept.
        if bytes.len() > MAX_ENTRY_BYTES {
          self.cache.invalidate(&path).await;
        }
        Ok(Some(bytes))
      }
      Err(err) if matches!(&*err, LoadError::Absent) => Ok(None),
      Err(err) => Err(ObjectCacheError(err)),
    }
  }
}
