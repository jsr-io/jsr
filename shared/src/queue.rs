// Copyright 2024 the JSR authors. All rights reserved. MIT license.
//! The queues background tasks are put on. They are Cloudflare Queues, which
//! deliver to the worker's `queue` handler (see `main.rs`), so there is one
//! only on the worker: natively (local development, tests) a task is run
//! inline by whatever would have queued it.

#[cfg(target_arch = "wasm32")]
pub use crate::worker_js::Queue;

/// The queue bound to the worker as `name`.
pub fn binding(name: &str) -> Option<Queue> {
  #[cfg(target_arch = "wasm32")]
  return Queue::new(name);
  #[cfg(not(target_arch = "wasm32"))]
  {
    let _ = name;
    None
  }
}

/// Never constructed natively.
#[cfg(not(target_arch = "wasm32"))]
#[derive(Clone)]
pub struct Queue(std::convert::Infallible);

#[cfg(not(target_arch = "wasm32"))]
impl Queue {
  pub async fn send(&self, _body: bytes::Bytes) -> Result<(), anyhow::Error> {
    match self.0 {}
  }
}
