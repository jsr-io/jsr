// Copyright 2024 the JSR authors. All rights reserved. MIT license.
//! Test harness shared by every crate's tests: an ephemeral database, buckets
//! on the fake S3 server, a few users and a scope, plus tarball fixtures.
//! Downstream crates get it through the `testing` feature.

use bytes::Bytes;
use flate2::Compression;
use flate2::write::GzEncoder;
use hyper::Body;
use hyper::Response;
use hyper::StatusCode;
use std::io::Write;
use std::net::Ipv4Addr;
use url::Url;

use crate::db::Database;
use crate::db::EphemeralDatabase;
use crate::ids::ScopeDescription;
use crate::models::NewGithubIdentity;
use crate::models::NewGitlabIdentity;
use crate::models::NewUser;
use crate::models::User;
use crate::s3::BucketWithQueue;
use crate::s3::Buckets;
use crate::s3::FakeS3Tester;
use crate::util::LicenseStore;

static SERVERS_STARTED: std::sync::OnceLock<()> = std::sync::OnceLock::new();
static LICENSE_STORE: std::sync::OnceLock<LicenseStore> =
  std::sync::OnceLock::new();

static TEST_INSTANCE_COUNTER: std::sync::atomic::AtomicU64 =
  std::sync::atomic::AtomicU64::new(0);

/// Ensure fake s3 server is running. The first call starts S3; subsequent calls return immediately.
fn ensure_servers_started() {
  SERVERS_STARTED.get_or_init(|| {
    FakeS3Tester::new();
  });
}

#[derive(Debug)]
pub struct TestUser {
  pub user: User,
  pub github_name: String,
}

/// An in-process stand-in for a fallback JSR registry, serving the handful of
/// artifacts the fallback path asks for: package metadata, version metadata,
/// and module files.
///
/// The fallback tests used to point at the real jsr.io, which made them
/// depend on network access and on whatever `@std/assert` happens to publish.
/// This serves a fixed `@std/assert@1.0.0` instead, so the tests assert the
/// fallback *mechanism* and nothing else.
pub struct FakeFallbackRegistry {
  url: Url,
  shutdown: Option<tokio::sync::oneshot::Sender<()>>,
}

impl FakeFallbackRegistry {
  pub async fn start() -> Self {
    let listener =
      std::net::TcpListener::bind((Ipv4Addr::LOCALHOST, 0)).unwrap();
    let addr = listener.local_addr().unwrap();

    let (tx, rx) = tokio::sync::oneshot::channel();
    let server = hyper::Server::from_tcp(listener)
      .unwrap()
      .serve(hyper::service::make_service_fn(|_| async {
        Ok::<_, hyper::Error>(hyper::service::service_fn(
          |req: hyper::Request<Body>| async move {
            Ok::<_, hyper::Error>(Self::respond(req.uri().path()))
          },
        ))
      }))
      .with_graceful_shutdown(async {
        let _ = rx.await;
      });
    tokio::spawn(server);

    Self {
      url: Url::parse(&format!("http://{addr}/")).unwrap(),
      shutdown: Some(tx),
    }
  }

  pub fn url(&self) -> Url {
    self.url.clone()
  }

  fn respond(path: &str) -> Response<Body> {
    let (content_type, body) = match path {
      "/@std/assert/meta.json" => (
        "application/json",
        r#"{
            "scope": "std",
            "name": "assert",
            "latest": "1.0.0",
            "versions": { "1.0.0": { "createdAt": "2024-01-01T00:00:00Z" } }
          }"#
          .to_owned(),
      ),
      "/@std/assert/1.0.0_meta.json" => (
        "application/json",
        r#"{
            "manifest": {},
            "moduleGraph2": {},
            "exports": { ".": "./mod.ts" }
          }"#
          .to_owned(),
      ),
      "/@std/assert/1.0.0/mod.ts" => (
        "text/typescript",
        "export function assert(cond: unknown): asserts cond {\n  if (!cond) throw new Error(\"assertion failed\");\n}\n"
          .to_owned(),
      ),
      _ => {
        return Response::builder()
          .status(StatusCode::NOT_FOUND)
          .body(Body::from("not found"))
          .unwrap();
      }
    };

    Response::builder()
      .status(StatusCode::OK)
      .header(hyper::header::CONTENT_TYPE, content_type)
      .body(Body::from(body))
      .unwrap()
  }
}

impl Drop for FakeFallbackRegistry {
  fn drop(&mut self) {
    if let Some(tx) = self.shutdown.take() {
      let _ = tx.send(());
    }
  }
}

/// A loopback URL with nothing listening on it, so connections are refused
/// immediately. For exercising the "fallback registry is down" path without
/// waiting on a DNS or connect timeout.
pub fn unreachable_fallback_url() -> Url {
  let listener = std::net::TcpListener::bind((Ipv4Addr::LOCALHOST, 0)).unwrap();
  let addr = listener.local_addr().unwrap();
  drop(listener);
  Url::parse(&format!("http://{addr}/")).unwrap()
}

/// An ephemeral database, a set of buckets on the fake S3 server, and a few
/// users and a scope to publish into. Everything a publishing task needs;
/// the api crate layers its router and user tokens on top.
pub struct TestSetup {
  pub ephemeral_database: EphemeralDatabase,
  pub buckets: Buckets,
  pub license_store: LicenseStore,
  pub user1: TestUser,
  pub user2: TestUser,
  pub user3: TestUser,
  pub staff_user: TestUser,
  pub scope: crate::models::Scope,
  pub fallback_registry_url: Option<Url>,
}

impl TestSetup {
  pub async fn new() -> Self {
    Self::with_fallback_registry(None).await
  }

  /// Like [`TestSetup::new`], but with a fallback registry configured.
  pub async fn with_fallback_registry(
    fallback_registry_url: Option<Url>,
  ) -> Self {
    ensure_servers_started();
    let test_id =
      TEST_INSTANCE_COUNTER.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    let ephemeral_database = EphemeralDatabase::create().await;
    let db = ephemeral_database.database.clone().unwrap();
    let s3 = FakeS3Tester::new();
    let publishing_name = format!("publishing-{test_id}");
    let modules_name = format!("modules-{test_id}");
    let docs_name = format!("docs-{test_id}");
    let npm_name = format!("npm-{test_id}");
    let ticket_attachments_name = format!("ticket-attachments-{test_id}");
    let (
      publishing_bucket,
      modules_bucket,
      docs_bucket,
      npm_bucket,
      ticket_attachments_bucket,
    ) = tokio::join!(
      s3.create_bucket(&publishing_name),
      s3.create_bucket(&modules_name),
      s3.create_bucket(&docs_name),
      s3.create_bucket(&npm_name),
      s3.create_bucket(&ticket_attachments_name),
    );
    let buckets = Buckets {
      publishing_bucket: BucketWithQueue::new(publishing_bucket),
      modules_bucket: BucketWithQueue::new(modules_bucket),
      docs_bucket: BucketWithQueue::new(docs_bucket),
      npm_bucket: BucketWithQueue::new(npm_bucket),
      ticket_attachments_bucket: BucketWithQueue::new(
        ticket_attachments_bucket,
      ),
    };

    let user1 = Self::create_user(
      &db,
      NewUser {
        name: "User 1",
        email: None,
        avatar_url: "https://avatars0.githubusercontent.com/u/952?v=4",
        github_id: Some(101),
        gitlab_id: Some(101),
        is_blocked: false,
        is_staff: false,
      },
      "ry",
    )
    .await;

    let user2 = Self::create_user(
      &db,
      NewUser {
        name: "User 2",
        email: None,
        avatar_url: "",
        github_id: Some(102),
        gitlab_id: Some(102),
        is_blocked: false,
        is_staff: false,
      },
      "lucacasonato",
    )
    .await;

    let user3 = Self::create_user(
      &db,
      NewUser {
        name: "User 3",
        email: None,
        avatar_url: "",
        github_id: Some(103),
        gitlab_id: Some(103),
        is_blocked: false,
        is_staff: false,
      },
      "crowlkats",
    )
    .await;

    let staff_user = Self::create_user(
      &db,
      NewUser {
        name: "User 4",
        email: None,
        avatar_url: "",
        github_id: Some(104),
        gitlab_id: Some(104),
        is_blocked: false,
        is_staff: true,
      },
      "bartlomieju",
    )
    .await;

    let scope_name = "scope".try_into().unwrap();

    db.create_scope(
      &user1.user.id,
      false,
      &scope_name,
      user1.user.id,
      &ScopeDescription::default(),
    )
    .await
    .unwrap();
    let (scope, _, _) = db
      .update_scope_limits(
        &staff_user.user.id,
        &scope_name,
        Some(250),
        Some(200),
        Some(1000),
      )
      .await
      .unwrap();

    db.add_bad_word_for_test("somebadword").await.unwrap();

    let license_store = LICENSE_STORE
      .get_or_init(crate::util::license_store)
      .clone();

    Self {
      ephemeral_database,
      buckets,
      license_store,
      user1,
      user2,
      user3,
      staff_user,
      scope,
      fallback_registry_url,
    }
  }

  async fn create_user<'a>(
    db: &Database,
    new_user: NewUser<'a>,
    github_name: &str,
  ) -> TestUser {
    db.upsert_github_identity(NewGithubIdentity {
      github_id: new_user.github_id.unwrap(),
      access_token: None,
      access_token_expires_at: None,
      refresh_token: None,
      refresh_token_expires_at: None,
    })
    .await
    .unwrap();
    db.upsert_gitlab_identity(NewGitlabIdentity {
      gitlab_id: new_user.gitlab_id.unwrap(),
      access_token: None,
      access_token_expires_at: None,
      refresh_token: None,
    })
    .await
    .unwrap();

    let user = db.insert_user(new_user).await.unwrap();

    TestUser {
      user,
      github_name: github_name.to_string(),
    }
  }

  pub fn db(&self) -> Database {
    self.ephemeral_database.database.clone().unwrap()
  }

  pub fn buckets(&self) -> Buckets {
    self.buckets.clone()
  }

  pub fn license_store(&self) -> LicenseStore {
    self.license_store.clone()
  }

  pub fn registry_url(&self) -> Url {
    Url::parse("http://jsr-tests.test").unwrap()
  }

  pub fn npm_url(&self) -> Url {
    Url::parse("http://npm.jsr-tests.test").unwrap()
  }
}

// ---- tarball fixtures ----

pub fn create_mock_tarball(name: &str) -> Bytes {
  let mut tar_bytes = Vec::new();
  let mut tar = tar::Builder::new(&mut tar_bytes);
  tar
    .append_dir_all(
      "./",
      format!("{}/testdata/tarballs/{name}/", env!("CARGO_MANIFEST_DIR")),
    )
    .unwrap();
  tar.finish().unwrap();
  drop(tar);

  let mut gz_bytes = Vec::new();
  let mut encoder = GzEncoder::new(&mut gz_bytes, Compression::default());
  encoder.write_all(&tar_bytes).unwrap();
  encoder.finish().unwrap();

  Bytes::from(gz_bytes)
}

pub fn create_case_insensitive_mock_tarball() -> Bytes {
  let mut tar_bytes = Vec::new();
  let mut tar = tar::Builder::new(&mut tar_bytes);
  tar
    .append_dir_all(
      "./",
      concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/testdata/tarballs/case_insensitive/"
      ),
    )
    .unwrap();
  let mut file = std::fs::File::open(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/testdata/tarballs/case_insensitive/README.md"
  ))
  .unwrap();
  tar.append_file("./readme.md", &mut file).unwrap();
  tar.finish().unwrap();
  drop(tar);

  let mut gz_bytes = Vec::new();
  let mut encoder = GzEncoder::new(&mut gz_bytes, Compression::default());
  encoder.write_all(&tar_bytes).unwrap();
  encoder.finish().unwrap();

  Bytes::from(gz_bytes)
}

pub fn create_invalid_path_mock_tarball() -> Bytes {
  let mut tar_bytes = Vec::new();
  let mut tar = tar::Builder::new(&mut tar_bytes);
  tar
    .append_dir_all(
      "./",
      concat!(env!("CARGO_MANIFEST_DIR"), "/testdata/tarballs/ok/"),
    )
    .unwrap();
  let mut file = std::fs::File::open(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/testdata/tarballs/ok/mod.ts"
  ))
  .unwrap();
  tar.append_file("./CON.ts", &mut file).unwrap();
  tar.finish().unwrap();
  drop(tar);

  let mut gz_bytes = Vec::new();
  let mut encoder = GzEncoder::new(&mut gz_bytes, Compression::default());
  encoder.write_all(&tar_bytes).unwrap();
  encoder.finish().unwrap();

  gz_bytes.into()
}

#[cfg(test)]
#[tokio::test]
async fn harness_expectations() {
  let t = TestSetup::new().await;
  assert!(!t.user1.user.is_staff);
  assert!(!t.user2.user.is_staff);
  assert!(!t.user3.user.is_staff);
  assert!(t.staff_user.user.is_staff);
}
