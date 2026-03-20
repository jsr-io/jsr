// Copyright 2024 the JSR authors. All rights reserved. MIT license.

use hyper::Body;
use hyper::Request;
use hyper::Response;
use serde::Serialize;
use tikv_jemalloc_ctl as jemalloc_ctl;
use tracing::error;

use crate::api::ApiError;
use crate::iam::ReqIamExt;

#[derive(Serialize)]
struct MemStats {
  allocated_bytes: usize,
  active_bytes: usize,
  resident_bytes: usize,
  mapped_bytes: usize,
  metadata_bytes: usize,
  retained_bytes: usize,

  allocated_mb: f64,
  active_mb: f64,
  resident_mb: f64,
  mapped_mb: f64,
  metadata_mb: f64,
  retained_mb: f64,
}

fn bytes_to_mb(bytes: usize) -> f64 {
  bytes as f64 / (1024.0 * 1024.0)
}

pub async fn mem_stats_handler(
  req: Request<Body>,
) -> Result<Response<Body>, ApiError> {
  req.iam().check_admin_access()?;

  let stats = read_stats().map_err(|e| {
    error!("jemalloc stats error: {e}");
    ApiError::InternalServerError
  })?;

  let body = serde_json::to_string_pretty(&stats).unwrap();
  Ok(
    Response::builder()
      .header("content-type", "application/json")
      .body(Body::from(body))
      .unwrap(),
  )
}

fn read_stats() -> Result<MemStats, tikv_jemalloc_ctl::Error> {
  jemalloc_ctl::epoch::advance()?;

  let allocated = jemalloc_ctl::stats::allocated::read()?;
  let active = jemalloc_ctl::stats::active::read()?;
  let resident = jemalloc_ctl::stats::resident::read()?;
  let mapped = jemalloc_ctl::stats::mapped::read()?;
  let metadata = jemalloc_ctl::stats::metadata::read()?;
  let retained = jemalloc_ctl::stats::retained::read()?;

  Ok(MemStats {
    allocated_bytes: allocated,
    active_bytes: active,
    resident_bytes: resident,
    mapped_bytes: mapped,
    metadata_bytes: metadata,
    retained_bytes: retained,
    allocated_mb: bytes_to_mb(allocated),
    active_mb: bytes_to_mb(active),
    resident_mb: bytes_to_mb(resident),
    mapped_mb: bytes_to_mb(mapped),
    metadata_mb: bytes_to_mb(metadata),
    retained_mb: bytes_to_mb(retained),
  })
}

/// Dumps a jemalloc heap profile to a temp file, reads it back, and returns
/// the raw profile bytes in the response body. This avoids needing filesystem
/// access to the Cloud Run container.
pub async fn heap_profile_handler(
  req: Request<Body>,
) -> Result<Response<Body>, ApiError> {
  req.iam().check_admin_access()?;

  let timestamp = std::time::SystemTime::now()
    .duration_since(std::time::UNIX_EPOCH)
    .unwrap()
    .as_secs();
  let file_path = format!("/tmp/jsr_heap_{timestamp}.prof");

  // Do the mallctl call in a block so the raw pointer doesn't live across awaits.
  let result = {
    let path = std::ffi::CString::new(file_path.clone()).unwrap();
    // SAFETY: prof.dump expects a `const char *`. raw::write<T> passes
    // &value as newp, so T = *const c_char makes mallctl see a pointer to
    // the C string path. `path` is kept alive for the duration of the call.
    let ptr: *const std::ffi::c_char = path.as_ptr();
    unsafe { tikv_jemalloc_ctl::raw::write(b"prof.dump\0", ptr) }
  };

  match result {
    Ok(()) => {
      let profile_bytes = tokio::fs::read(&file_path).await.map_err(|e| {
        error!("failed to read heap profile at {file_path}: {e}");
        ApiError::InternalServerError
      })?;
      let _ = tokio::fs::remove_file(&file_path).await;

      Ok(
        Response::builder()
          .header("content-type", "application/octet-stream")
          .header(
            "content-disposition",
            format!("attachment; filename=\"jsr_heap_{timestamp}.prof\""),
          )
          .body(Body::from(profile_bytes))
          .unwrap(),
      )
    }
    Err(e) => {
      let body = serde_json::json!({
        "status": "error",
        "error": e.to_string(),
        "hint": "Profiling requires MALLOC_CONF=prof:true at startup",
      });
      Ok(
        Response::builder()
          .status(500)
          .header("content-type", "application/json")
          .body(Body::from(serde_json::to_string_pretty(&body).unwrap()))
          .unwrap(),
      )
    }
  }
}
