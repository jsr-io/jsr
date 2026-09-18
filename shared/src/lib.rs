// Copyright 2024 the JSR authors. All rights reserved. MIT license.

use url::Url;

pub const MAX_PUBLISH_TARBALL_SIZE: u64 = 20 * 1024 * 1024; // 20mb

pub struct RegistryUrl(pub Url);
pub struct NpmUrl(pub Url);
pub struct FallbackRegistryUrl(pub Option<Url>);

pub struct PublishQueue(pub Option<external::gcp::Queue>);

pub mod api;
pub mod db;
pub mod docs;
pub mod emails;
pub mod errors;
pub mod errors_internal;
pub mod external;
pub mod ids;
pub mod metadata;
pub mod models;
pub mod npm;
pub mod s3;
pub mod s3_paths;
pub mod task_queue;
pub mod util;

#[cfg(test)]
pub mod test;
