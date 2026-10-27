// Copyright 2024 the JSR authors. All rights reserved. MIT license.
use crate::db::*;
use crate::ids::PackageName;
use crate::ids::PackagePath;
use crate::ids::ScopeDescription;
use crate::ids::ScopeName;
use crate::ids::Version;
use crate::npm::NPM_TARBALL_REVISION;
use chrono::DateTime;
use chrono::Utc;

#[tokio::test]
async fn publishing_tasks() {
  let db = EphemeralDatabase::create().await;

  let user_id = uuid::Uuid::default();
  let scope_name = "scope".try_into().unwrap();
  let package_name = "package".try_into().unwrap();
  let version = "1.0.0".try_into().unwrap();
  let config_file = "/jsr.json".try_into().unwrap();

  let _scope = db
    .create_scope(
      &user_id,
      false,
      &scope_name,
      user_id,
      &ScopeDescription::default(),
    )
    .await
    .unwrap();
  let res = db.create_package(&scope_name, &package_name).await.unwrap();
  assert!(matches!(res, CreatePackageResult::Ok(_)));

  let CreatePublishingTaskResult::Created((pt, _)) = db
    .create_publishing_task(NewPublishingTask {
      user_id: Some(user_id),
      package_scope: &scope_name,
      package_name: &package_name,
      package_version: &version,
      config_file: &config_file,
    })
    .await
    .unwrap()
  else {
    unreachable!() // doesn't conflict with existing tasks
  };
  assert_eq!(pt.status, PublishingTaskStatus::Pending);

  let res = db
    .create_publishing_task(NewPublishingTask {
      user_id: Some(user_id),
      package_scope: &scope_name,
      package_name: &package_name,
      package_version: &version,
      config_file: &config_file,
    })
    .await
    .unwrap();
  let CreatePublishingTaskResult::Exists((pt_reused, _)) = res else {
    unreachable!() // does conflict with existing task
  };
  assert_eq!(pt_reused.id, pt.id);

  let (pt2, _) = db.get_publishing_task(pt.id).await.unwrap().unwrap();
  assert_eq!(pt2.id, pt2.id);
  assert_eq!(pt2.package_scope, scope_name);
  assert_eq!(pt2.package_name, package_name);
  assert_eq!(pt2.status, PublishingTaskStatus::Pending);
  assert!(pt.error.is_none());

  let no_pt = db.get_publishing_task(uuid::Uuid::new_v4()).await.unwrap();
  assert!(no_pt.is_none());

  let pt3 = db
    .update_publishing_task_status(
      None,
      pt.id,
      PublishingTaskStatus::Pending,
      PublishingTaskStatus::Failure,
      Some(PublishingTaskError {
        code: "invalidConfigFile".to_string(),
        message: "Your config file is invalid.".to_string(),
      }),
    )
    .await
    .unwrap();
  let error = pt3.error.unwrap();
  assert_eq!(error.code, "invalidConfigFile");
  assert_eq!(error.message, "Your config file is invalid.");

  let CreatePublishingTaskResult::Created((pt4, _)) = db
    .create_publishing_task(NewPublishingTask {
      user_id: Some(user_id),
      package_scope: &scope_name,
      package_name: &package_name,
      package_version: &version,
      config_file: &config_file,
    })
    .await
    .unwrap()
  else {
    unreachable!() // does not conflict with existing task because it has a validation error
  };

  db.update_publishing_task_status(
    None,
    pt4.id,
    PublishingTaskStatus::Pending,
    PublishingTaskStatus::Success,
    None,
  )
  .await
  .unwrap();

  let (pt5, _) = db.get_publishing_task(pt4.id).await.unwrap().unwrap();
  assert_eq!(pt5.status, PublishingTaskStatus::Success);
  assert!(pt5.updated_at > pt5.created_at);
}

#[tokio::test]
async fn claim_stale_publishing_tasks() {
  let db = EphemeralDatabase::create().await;

  let user_id = uuid::Uuid::default();
  let scope_name: ScopeName = "scope".try_into().unwrap();
  let package_name: PackageName = "package".try_into().unwrap();
  let config_file: PackagePath = "/jsr.json".try_into().unwrap();

  db.create_scope(
    &user_id,
    false,
    &scope_name,
    user_id,
    &ScopeDescription::default(),
  )
  .await
  .unwrap();
  db.create_package(&scope_name, &package_name).await.unwrap();

  // Create one task per terminal/non-terminal status (each on its own version,
  // since `create_publishing_task` only allows one non-failure task per
  // version) and drive it to the desired status via direct transitions.
  let mut ids = std::collections::HashMap::new();
  for (version_str, target) in [
    ("1.0.0", PublishingTaskStatus::Pending),
    ("2.0.0", PublishingTaskStatus::Processing),
    ("3.0.0", PublishingTaskStatus::Processed),
    ("4.0.0", PublishingTaskStatus::Success),
    ("5.0.0", PublishingTaskStatus::Failure),
  ] {
    let version: Version = version_str.try_into().unwrap();
    let CreatePublishingTaskResult::Created((pt, _)) = db
      .create_publishing_task(NewPublishingTask {
        user_id: Some(user_id),
        package_scope: &scope_name,
        package_name: &package_name,
        package_version: &version,
        config_file: &config_file,
      })
      .await
      .unwrap()
    else {
      unreachable!()
    };

    // Walk the status machine from `pending` up to the target status.
    let path: &[PublishingTaskStatus] = match target {
      PublishingTaskStatus::Pending => &[],
      PublishingTaskStatus::Processing => &[PublishingTaskStatus::Processing],
      PublishingTaskStatus::Processed => &[
        PublishingTaskStatus::Processing,
        PublishingTaskStatus::Processed,
      ],
      PublishingTaskStatus::Success => &[
        PublishingTaskStatus::Processing,
        PublishingTaskStatus::Processed,
        PublishingTaskStatus::Success,
      ],
      PublishingTaskStatus::Failure => &[PublishingTaskStatus::Failure],
    };
    let mut prev = PublishingTaskStatus::Pending;
    for next in path {
      let error =
        (*next == PublishingTaskStatus::Failure).then(|| PublishingTaskError {
          code: "x".to_string(),
          message: "x".to_string(),
        });
      db.update_publishing_task_status(None, pt.id, prev, next.clone(), error)
        .await
        .unwrap();
      prev = next.clone();
    }
    ids.insert(version_str, pt.id);
  }

  let get_status = |id| {
    let db = &db;
    async move { db.get_publishing_task(id).await.unwrap().unwrap().0.status }
  };

  for id in ids.values() {
    db.backdate_publishing_task(*id, 3600, 3600).await.unwrap();
  }
  let stale = db.claim_stale_publishing_tasks(1800, 86400).await.unwrap();
  let claimed: std::collections::HashSet<_> =
    stale.claimed.iter().copied().collect();
  assert!(stale.abandoned.is_empty(), "{stale:?}");
  assert_eq!(
    claimed,
    [ids["1.0.0"], ids["2.0.0"], ids["3.0.0"]]
      .into_iter()
      .collect(),
    "{stale:?}"
  );
  assert_eq!(
    get_status(ids["1.0.0"]).await,
    PublishingTaskStatus::Pending
  );
  assert_eq!(
    get_status(ids["2.0.0"]).await,
    PublishingTaskStatus::Pending
  );
  assert_eq!(
    get_status(ids["3.0.0"]).await,
    PublishingTaskStatus::Processed
  );

  let stale = db.claim_stale_publishing_tasks(1800, 86400).await.unwrap();
  assert!(stale.claimed.is_empty(), "{stale:?}");
  assert!(stale.abandoned.is_empty(), "{stale:?}");

  db.update_publishing_task_status(
    None,
    ids["2.0.0"],
    PublishingTaskStatus::Pending,
    PublishingTaskStatus::Processing,
    None,
  )
  .await
  .unwrap();
  for id in ids.values() {
    db.backdate_publishing_task(*id, 2 * 86400, 3600)
      .await
      .unwrap();
  }
  let stale = db.claim_stale_publishing_tasks(1800, 86400).await.unwrap();
  let abandoned: std::collections::HashSet<_> =
    stale.abandoned.iter().copied().collect();
  assert_eq!(
    abandoned,
    [ids["1.0.0"], ids["2.0.0"]].into_iter().collect(),
    "{stale:?}"
  );
  assert_eq!(stale.claimed, vec![ids["3.0.0"]], "{stale:?}");
  assert_eq!(
    get_status(ids["1.0.0"]).await,
    PublishingTaskStatus::Failure
  );
  assert_eq!(
    get_status(ids["2.0.0"]).await,
    PublishingTaskStatus::Failure
  );
  assert_eq!(
    get_status(ids["4.0.0"]).await,
    PublishingTaskStatus::Success
  );
}

#[tokio::test]
async fn create_publishing_task_auto_fails_stranded_processing() {
  let db = EphemeralDatabase::create().await;

  let user_id = uuid::Uuid::default();
  let scope_name: ScopeName = "scope".try_into().unwrap();
  let package_name: PackageName = "package".try_into().unwrap();
  let config_file: PackagePath = "/jsr.json".try_into().unwrap();

  db.create_scope(
    &user_id,
    false,
    &scope_name,
    user_id,
    &ScopeDescription::default(),
  )
  .await
  .unwrap();
  db.create_package(&scope_name, &package_name).await.unwrap();

  // Helper: create a task on `version_str` and walk it up to `target`.
  let make_task = async |version_str: &str, target: PublishingTaskStatus| {
    let version: Version = version_str.try_into().unwrap();
    let CreatePublishingTaskResult::Created((pt, _)) = db
      .create_publishing_task(NewPublishingTask {
        user_id: Some(user_id),
        package_scope: &scope_name,
        package_name: &package_name,
        package_version: &version,
        config_file: &config_file,
      })
      .await
      .unwrap()
    else {
      unreachable!()
    };
    let path: &[PublishingTaskStatus] = match target {
      PublishingTaskStatus::Pending => &[],
      PublishingTaskStatus::Processing => &[PublishingTaskStatus::Processing],
      PublishingTaskStatus::Processed => &[
        PublishingTaskStatus::Processing,
        PublishingTaskStatus::Processed,
      ],
      _ => unreachable!(),
    };
    let mut prev = PublishingTaskStatus::Pending;
    for next in path {
      db.update_publishing_task_status(None, pt.id, prev, next.clone(), None)
        .await
        .unwrap();
      prev = next.clone();
    }
    pt.id
  };

  // 1. Fresh `processing` task: a duplicate publish gets the existing task back.
  let fresh_id = make_task("1.0.0", PublishingTaskStatus::Processing).await;
  let v1: Version = "1.0.0".try_into().unwrap();
  let res = db
    .create_publishing_task(NewPublishingTask {
      user_id: Some(user_id),
      package_scope: &scope_name,
      package_name: &package_name,
      package_version: &v1,
      config_file: &config_file,
    })
    .await
    .unwrap();
  let CreatePublishingTaskResult::Exists((reused, _)) = res else {
    panic!("fresh processing task should return Exists")
  };
  assert_eq!(reused.id, fresh_id);
  assert_eq!(reused.status, PublishingTaskStatus::Processing);

  // 2. Stranded `processing` task (>30 min old): inline auto-fail, new task created.
  let stranded_id = make_task("2.0.0", PublishingTaskStatus::Processing).await;
  db.backdate_publishing_task_updated_at(stranded_id, 31 * 60)
    .await
    .unwrap();
  let v2: Version = "2.0.0".try_into().unwrap();
  let res = db
    .create_publishing_task(NewPublishingTask {
      user_id: Some(user_id),
      package_scope: &scope_name,
      package_name: &package_name,
      package_version: &v2,
      config_file: &config_file,
    })
    .await
    .unwrap();
  let CreatePublishingTaskResult::Created((new_task, _)) = res else {
    panic!(
      "stranded processing task should be auto-failed and new task created"
    )
  };
  assert_ne!(new_task.id, stranded_id);
  assert_eq!(new_task.status, PublishingTaskStatus::Pending);
  let (old, _) = db.get_publishing_task(stranded_id).await.unwrap().unwrap();
  assert_eq!(old.status, PublishingTaskStatus::Failure);
  assert_eq!(old.error.unwrap().code, "stale");

  // 3. Stranded `processed` task: still returns Exists (left to the reaper).
  let processed_id = make_task("3.0.0", PublishingTaskStatus::Processed).await;
  db.backdate_publishing_task_updated_at(processed_id, 31 * 60)
    .await
    .unwrap();
  let v3: Version = "3.0.0".try_into().unwrap();
  let res = db
    .create_publishing_task(NewPublishingTask {
      user_id: Some(user_id),
      package_scope: &scope_name,
      package_name: &package_name,
      package_version: &v3,
      config_file: &config_file,
    })
    .await
    .unwrap();
  let CreatePublishingTaskResult::Exists((reused, _)) = res else {
    panic!(
      "stranded processed task is the reaper's responsibility, not this path"
    )
  };
  assert_eq!(reused.id, processed_id);
  assert_eq!(reused.status, PublishingTaskStatus::Processed);
}

#[tokio::test]
async fn users() {
  let db = EphemeralDatabase::create().await;

  let new_user = NewUser {
    name: "Staff",
    email: None,
    avatar_url: "",
    github_id: None,
    gitlab_id: None,
    is_blocked: false,
    is_staff: true,
  };
  let staff_user = db.insert_user(new_user).await.unwrap();

  let new_user = NewUser {
    name: "Alice",
    email: Some("alice@example.com"),
    avatar_url: "https://example.com/alice.png",
    github_id: None,
    gitlab_id: None,
    is_blocked: false,
    is_staff: true,
  };
  let user = db.insert_user(new_user).await.unwrap();
  assert_eq!(user.name, "Alice");
  assert_eq!(user.avatar_url, "https://example.com/alice.png");
  assert_eq!(user.email, Some("alice@example.com".to_string()));
  assert_eq!(user.scope_limit, Some(3));
  assert!(user.is_staff);
  assert!(!user.is_blocked);

  let user_ = db
    .user_set_staff(&staff_user.id, user.id, false)
    .await
    .unwrap();
  assert!(!user_.is_staff);

  let user_ = db
    .user_set_staff(&staff_user.id, user.id, true)
    .await
    .unwrap();
  assert!(user_.is_staff);

  let user_ = db
    .user_set_blocked(&staff_user.id, user.id, true)
    .await
    .unwrap();
  assert!(user_.is_blocked);

  let user_ = db
    .user_set_blocked(&staff_user.id, user.id, false)
    .await
    .unwrap();
  assert!(!user_.is_blocked);

  let user2 = db.get_user(user.id).await.unwrap().unwrap();
  assert_eq!(user2.id, user.id);
  assert_eq!(user2.name, "Alice");
  assert_eq!(user2.avatar_url, "https://example.com/alice.png");
  assert_eq!(user2.email, Some("alice@example.com".to_string()));

  let no_user = db.get_user(uuid::Uuid::new_v4()).await.unwrap();
  assert!(no_user.is_none());

  let (total_users, users) = db.list_users(0, 20, None, None).await.unwrap();
  assert_eq!(total_users, 3);
  assert_eq!(users.len(), 3);
  assert_eq!(users[0].id, user.id);
  assert_eq!(users[0].name, "Alice");
  assert_eq!(users[0].avatar_url, "https://example.com/alice.png");
  assert_eq!(users[0].email, Some("alice@example.com".to_string()));
  assert_eq!(users[0].scope_usage, 0);
  assert_eq!(users[2].id, uuid::Uuid::default()); // added by migrations

  // A deletion hold is enforced inside delete_user itself, not just by the
  // handlers' pre-checks.
  db.user_set_deletion_hold(&staff_user.id, user.id, true)
    .await
    .unwrap();
  let held = db.delete_user(&user.id, false, user.id).await.unwrap();
  assert_eq!(held, UserDeleteResult::Held);
  assert!(db.get_user(user.id).await.unwrap().is_some());
  db.user_set_deletion_hold(&staff_user.id, user.id, false)
    .await
    .unwrap();

  let deleted = db.delete_user(&user.id, false, user.id).await.unwrap();
  assert_eq!(deleted, UserDeleteResult::Deleted);

  let no_user = db.get_user(user.id).await.unwrap();
  assert!(no_user.is_none());

  let (total_users, users) = db.list_users(0, 20, None, None).await.unwrap();
  assert_eq!(total_users, 2);
  assert_eq!(users.len(), 2); // just the default user added by migrations

  let no_user = db.delete_user(&user.id, false, user.id).await.unwrap();
  assert_eq!(no_user, UserDeleteResult::NotFound);
}

#[tokio::test]
async fn packages() {
  let db = EphemeralDatabase::create().await;

  let alice = db
    .insert_user(NewUser {
      name: "Alice",
      email: None,
      avatar_url: "https://example.com/alice.png",
      github_id: None,
      gitlab_id: None,
      is_blocked: false,
      is_staff: false,
    })
    .await
    .unwrap();

  let scope_name = "scope".try_into().unwrap();
  let package_name = "testpkg".try_into().unwrap();

  db.create_scope(
    &alice.id,
    false,
    &scope_name,
    alice.id,
    &ScopeDescription::default(),
  )
  .await
  .unwrap();

  let alice2 = db.get_user(alice.id).await.unwrap().unwrap();
  assert_eq!(alice2.scope_usage, 1);

  assert!(
    db.get_scope_member(&scope_name, alice.id)
      .await
      .unwrap()
      .is_some()
  );

  let CreatePackageResult::Ok(package) =
    db.create_package(&scope_name, &package_name).await.unwrap()
  else {
    unreachable!()
  };
  assert_eq!(package.scope, scope_name);
  assert_eq!(package.name, package_name);

  // calling create_package now should error
  let CreatePackageResult::AlreadyExists =
    db.create_package(&scope_name, &package_name).await.unwrap()
  else {
    unreachable!()
  };

  let (package2, _, _) = db
    .get_package(&scope_name, &package_name)
    .await
    .unwrap()
    .unwrap();
  assert_eq!(package2.scope, scope_name);
  assert_eq!(package2.name, package_name);

  let bad_name = "badname".try_into().unwrap();
  let no_package = db.get_package(&scope_name, &bad_name).await.unwrap();
  assert!(no_package.is_none());

  let (total, packages) = db
    .list_packages_by_scope(&scope_name, false, 0, 100)
    .await
    .unwrap();
  assert_eq!(total, 1);
  assert_eq!(packages.len(), 1);
  assert_eq!(packages[0].0.name, package_name);
  assert!(packages[0].1.is_none());
}

#[tokio::test]
async fn scope_members() {
  let db = EphemeralDatabase::create().await;

  let bob = db
    .insert_user(NewUser {
      name: "Bob",
      email: None,
      avatar_url: "https://example.com/bob.png",
      github_id: None,
      gitlab_id: None,
      is_blocked: false,
      is_staff: false,
    })
    .await
    .unwrap();

  let scope_name = "scope".try_into().unwrap();

  db.create_scope(
    &bob.id,
    false,
    &scope_name,
    bob.id,
    &ScopeDescription::default(),
  )
  .await
  .unwrap();

  let scope = db
    .get_scope(&ScopeName::try_from("scope").unwrap())
    .await
    .unwrap()
    .unwrap();
  assert_eq!(scope.creator, bob.id);

  let alice = db
    .insert_user(NewUser {
      name: "Alice",
      email: None,
      avatar_url: "https://example.com/alice.png",
      github_id: None,
      gitlab_id: None,
      is_blocked: false,
      is_staff: false,
    })
    .await
    .unwrap();

  let alice_member = db
    .add_user_to_scope(NewScopeMember {
      scope: &scope_name,
      user_id: alice.id,
      is_admin: false,
    })
    .await
    .unwrap();
  assert_eq!(alice_member.user_id, alice.id);

  let members = db.list_scope_members(&scope_name).await.unwrap();
  assert_eq!(members.len(), 2);
  assert_eq!(members[0].0.user_id, alice.id);
  assert_eq!(members[0].1.id, alice.id);
  assert_eq!(members[1].0.user_id, bob.id);
  assert_eq!(members[1].1.id, bob.id);

  db.delete_scope_member(&scope_name, alice.id).await.unwrap();
  let members = db.list_scope_members(&scope_name).await.unwrap();
  assert_eq!(members.len(), 1);
  assert_eq!(members[0].0.user_id, bob.id);
}

#[tokio::test]
async fn create_package_version_and_finalize_publishing_task() {
  let db = EphemeralDatabase::create().await;

  let scope = ScopeName::try_from("scope").unwrap();
  let package_name = PackageName::try_from("foo").unwrap();

  let bob = db
    .insert_user(NewUser {
      name: "Bob",
      email: None,
      github_id: None,
      gitlab_id: None,
      is_blocked: false,
      is_staff: false,
      avatar_url: "https://example.com/bob.png",
    })
    .await
    .unwrap();

  db.create_scope(&bob.id, false, &scope, bob.id, &ScopeDescription::default())
    .await
    .unwrap();

  let CreatePackageResult::Ok(_package) =
    db.create_package(&scope, &package_name).await.unwrap()
  else {
    unreachable!()
  };

  let version = Version::try_from("1.2.3").unwrap();
  let config_file = PackagePath::try_from("/jsr.json").unwrap();
  let CreatePublishingTaskResult::Created((task, _)) = db
    .create_publishing_task(NewPublishingTask {
      user_id: Some(bob.id),
      package_scope: &scope,
      package_name: &package_name,
      package_version: &version,
      config_file: &config_file,
    })
    .await
    .unwrap()
  else {
    unreachable!()
  };

  db.update_publishing_task_status(
    None,
    task.id,
    PublishingTaskStatus::Pending,
    PublishingTaskStatus::Processing,
    None,
  )
  .await
  .unwrap();

  let package_files = vec![];
  let package_version_dependencies = vec![];
  let npm_tarball = NewNpmTarball {
    scope: &scope,
    name: &package_name,
    version: &version,
    revision: NPM_TARBALL_REVISION as i32,
    sha1: "",
    sha512: "",
    size: 0,
  };

  let task = db
    .create_package_version_and_npm_tarball_and_finalize_publishing_task(
      task.id,
      NewPackageVersion {
        scope: &scope,
        name: &package_name,
        version: &version,
        user_id: None,
        readme_path: None,
        uses_npm: true,
        exports: &ExportsMap::mock(),
        meta: Default::default(),
        license: "MIT".to_string(),
      },
      &package_files,
      &package_version_dependencies,
      npm_tarball,
    )
    .await
    .unwrap();
  assert_eq!(task.status, PublishingTaskStatus::Processed);

  let pv = db
    .get_package_version(&scope, &package_name, &version)
    .await
    .unwrap()
    .unwrap();
  assert!(pv.uses_npm);
  assert_eq!(pv.readme_path, None);
  assert_eq!(pv.user_id, None);

  let task = db
    .update_publishing_task_status(
      None,
      task.id,
      PublishingTaskStatus::Processed,
      PublishingTaskStatus::Success,
      None,
    )
    .await
    .unwrap();
  assert_eq!(task.status, PublishingTaskStatus::Success);
}

#[tokio::test]
async fn package_version_with_large_meta() {
  let db = EphemeralDatabase::create().await;

  let user_id = uuid::Uuid::default();
  let scope_name: ScopeName = "scope".try_into().unwrap();
  let package_name: PackageName = "testpkg".try_into().unwrap();
  let version: Version = "1.0.0".try_into().unwrap();

  db.create_scope(
    &user_id,
    false,
    &scope_name,
    user_id,
    &ScopeDescription::default(),
  )
  .await
  .unwrap();
  let CreatePackageResult::Ok(package) =
    db.create_package(&scope_name, &package_name).await.unwrap()
  else {
    unreachable!()
  };

  // Regression test for jsr-io/jsr#1505: `meta` used to be an INCLUDE column
  // of idx_package_versions_latest, so a meta larger than Postgres' ~8KB
  // b-tree index row cap (e.g. thousands of undocumented entrypoints) made
  // this insert fail, permanently wedging the publishing task. The version
  // must be non-prerelease and non-yanked so the row lands in that partial
  // index.
  let meta = PackageVersionMeta {
    entrypoints_without_docs: (0..20_000)
      .map(|i| format!("./entrypoint{i}"))
      .collect(),
    ..Default::default()
  };
  db.create_package_version_for_test(NewPackageVersion {
    scope: &package.scope,
    name: &package.name,
    version: &version,
    user_id: None,
    readme_path: None,
    exports: &ExportsMap::mock(),
    uses_npm: false,
    meta,
    license: "MIT".to_string(),
  })
  .await
  .unwrap();
}

#[tokio::test]
async fn package_files() {
  let db = EphemeralDatabase::create().await;

  let user = db
    .insert_user(NewUser {
      name: "Alice",
      email: None,
      avatar_url: "https://example.com/alice.png",
      github_id: None,
      gitlab_id: None,
      is_blocked: false,
      is_staff: false,
    })
    .await
    .unwrap();

  let scope_name = "scope".try_into().unwrap();
  let package_name = "testpkg".try_into().unwrap();
  let version = "1.2.3".try_into().unwrap();

  db.create_scope(
    &user.id,
    false,
    &scope_name,
    user.id,
    &ScopeDescription::default(),
  )
  .await
  .unwrap();

  let CreatePackageResult::Ok(package) =
    db.create_package(&scope_name, &package_name).await.unwrap()
  else {
    unreachable!()
  };

  let package_version = db
    .create_package_version_for_test(NewPackageVersion {
      scope: &package.scope,
      name: &package.name,
      version: &version,
      user_id: None,
      readme_path: None,
      exports: &ExportsMap::mock(),
      uses_npm: false,
      meta: Default::default(),
      license: "MIT".to_string(),
    })
    .await
    .unwrap();

  let checksum =
    "sha256:6dcd4ce23d88e2ee95838f7b014b6284f0f9a8c3c6c7f1625b6a40b59f4777b1";
  let path = PackagePath::try_from("/jsr.json").unwrap();
  let package_file = db
    .create_package_file_for_test(NewPackageFile {
      scope: &package_version.scope,
      name: &package_version.name,
      version: &package_version.version,
      path: &path,
      size: 1024,
      checksum: Some(checksum),
    })
    .await
    .unwrap();
  assert_eq!(package_file.path, path);
  assert_eq!(package_file.size, 1024);

  let package_files = db
    .list_package_files(&scope_name, &package_name, &version)
    .await
    .unwrap();
  assert_eq!(package_files.len(), 1);
  assert_eq!(package_files[0].scope, scope_name);
  assert_eq!(package_files[0].name, package_name);
  assert_eq!(package_files[0].version, version);
  assert_eq!(package_files[0].path, path);
  assert_eq!(package_files[0].size, 1024);
  assert_eq!(package_files[0].checksum.as_ref().unwrap(), checksum);

  let other_scope_name = "otherscope".try_into().unwrap();

  let package_files = db
    .list_package_files(&other_scope_name, &package_name, &version)
    .await
    .unwrap();
  assert_eq!(package_files.len(), 0);
}

#[tokio::test]
async fn oauth_state() {
  let db = EphemeralDatabase::create().await;

  let new_oauth_state = NewOauthState {
    csrf_token: "a",
    pkce_code_verifier: "b",
    redirect_url: "c",
    user_id: None,
  };
  let oauth_state = db.insert_oauth_state(new_oauth_state).await.unwrap();
  assert_eq!(oauth_state.csrf_token, "a");
  assert_eq!(oauth_state.pkce_code_verifier, "b");
  assert_eq!(oauth_state.redirect_url, "c");
  assert_eq!(oauth_state.user_id, None);

  let oauth_state2 = db
    .get_oauth_state(&oauth_state.csrf_token)
    .await
    .unwrap()
    .unwrap();
  assert_eq!(oauth_state2.csrf_token, "a");
  assert_eq!(oauth_state2.pkce_code_verifier, "b");
  assert_eq!(oauth_state2.redirect_url, "c");

  // A state from the "connect" (account-linking) flow is bound to the user that
  // initiated it; the binding must round-trip so the callback can enforce it.
  let user_id: uuid::Uuid =
    "00000000-0000-0000-0000-000000000000".try_into().unwrap();
  let bound = db
    .insert_oauth_state(NewOauthState {
      csrf_token: "d",
      pkce_code_verifier: "e",
      redirect_url: "f",
      user_id: Some(user_id),
    })
    .await
    .unwrap();
  assert_eq!(bound.user_id, Some(user_id));
  assert_eq!(
    db.get_oauth_state("d").await.unwrap().unwrap().user_id,
    Some(user_id)
  );

  let oauth_state3 = db
    .delete_oauth_state(&oauth_state.csrf_token)
    .await
    .unwrap()
    .unwrap();
  assert_eq!(oauth_state3.csrf_token, "a");
  assert_eq!(oauth_state3.pkce_code_verifier, "b");
  assert_eq!(oauth_state3.redirect_url, "c");

  assert!(
    db.delete_oauth_state(&oauth_state.csrf_token)
      .await
      .unwrap()
      .is_none()
  )
}

#[tokio::test]
async fn tokens() {
  let db = EphemeralDatabase::create().await;

  let new_user = NewUser {
    name: "Alice",
    email: Some("alice@example.com"),
    avatar_url: "https://example.com/alice.png",
    github_id: None,
    gitlab_id: None,
    is_blocked: false,
    is_staff: false,
  };
  let user = db.insert_user(new_user).await.unwrap();

  let time = DateTime::<Utc>::default();
  let new_token = NewToken {
    hash: "0".to_string(),
    user_id: user.id,
    r#type: TokenType::Web,
    description: None,
    expires_at: Some(time),
    permissions: None,
  };
  let token = db.insert_token(new_token).await.unwrap();
  assert_eq!(token.hash, "0");
  assert_eq!(token.user_id, user.id);
  assert_eq!(token.r#type, TokenType::Web);
  assert_eq!(token.description, None);
  assert_eq!(token.expires_at, Some(time));

  let token2 = db.get_token_by_hash(&token.hash).await.unwrap().unwrap();
  assert_eq!(token2.id, token.id);

  let no_token = db.get_token_by_hash("1").await.unwrap();
  assert!(no_token.is_none());
}

// The tests below run the queries that no other test reaches, against rows,
// so the SQL, the parameter types and the row mappers all get checked. There
// is no compile-time checking of queries, so a new query needs a test that
// runs it.

struct Seed {
  db: EphemeralDatabase,
  bob: User,
  alice: User,
  scope: ScopeName,
  name: PackageName,
  version: Version,
}

/// A staff user (bob, with GitHub and GitLab identities) owning a scope with
/// a package that has one published version, an invite to that scope for a
/// second user (alice), and some downloads of the version.
async fn seed() -> Seed {
  let db = EphemeralDatabase::create().await;
  let scope = ScopeName::try_from("scope").unwrap();
  let name = PackageName::try_from("foo").unwrap();
  let version = Version::try_from("1.2.3").unwrap();

  for github_id in [101, 102, 103] {
    db.upsert_github_identity(NewGithubIdentity {
      github_id,
      access_token: Some("token".to_string()),
      access_token_expires_at: Some(Utc::now()),
      refresh_token: None,
      refresh_token_expires_at: None,
    })
    .await
    .unwrap();
  }
  for gitlab_id in [201, 202, 203] {
    db.upsert_gitlab_identity(NewGitlabIdentity {
      gitlab_id,
      access_token: Some("token".to_string()),
      access_token_expires_at: Some(Utc::now()),
      refresh_token: None,
    })
    .await
    .unwrap();
  }

  let bob = db
    .insert_user(NewUser {
      name: "Bob",
      email: Some("bob@example.com"),
      avatar_url: "https://example.com/bob.png",
      github_id: Some(101),
      gitlab_id: Some(201),
      is_blocked: false,
      is_staff: true,
    })
    .await
    .unwrap();
  let alice = db
    .insert_user(NewUser {
      name: "Alice",
      email: None,
      avatar_url: "https://example.com/alice.png",
      github_id: None,
      gitlab_id: None,
      is_blocked: false,
      is_staff: false,
    })
    .await
    .unwrap();

  db.create_scope(&bob.id, false, &scope, bob.id, &ScopeDescription::default())
    .await
    .unwrap();
  let CreatePackageResult::Ok(_) =
    db.create_package(&scope, &name).await.unwrap()
  else {
    unreachable!()
  };

  let config_file = PackagePath::try_from("/jsr.json").unwrap();
  let CreatePublishingTaskResult::Created((task, _)) = db
    .create_publishing_task(NewPublishingTask {
      user_id: Some(bob.id),
      package_scope: &scope,
      package_name: &name,
      package_version: &version,
      config_file: &config_file,
    })
    .await
    .unwrap()
  else {
    unreachable!()
  };
  db.update_publishing_task_status(
    None,
    task.id,
    PublishingTaskStatus::Pending,
    PublishingTaskStatus::Processing,
    None,
  )
  .await
  .unwrap();
  db.create_package_version_and_npm_tarball_and_finalize_publishing_task(
    task.id,
    NewPackageVersion {
      scope: &scope,
      name: &name,
      version: &version,
      user_id: Some(&bob.id),
      readme_path: None,
      uses_npm: true,
      exports: &ExportsMap::mock(),
      meta: Default::default(),
      license: "MIT".to_string(),
    },
    &[],
    &[],
    NewNpmTarball {
      scope: &scope,
      name: &name,
      version: &version,
      revision: NPM_TARBALL_REVISION as i32,
      sha1: "sha1",
      sha512: "sha512",
      size: 1,
    },
  )
  .await
  .unwrap();
  db.update_publishing_task_status(
    None,
    task.id,
    PublishingTaskStatus::Processed,
    PublishingTaskStatus::Success,
    None,
  )
  .await
  .unwrap();

  db.add_scope_invite(
    &bob.id,
    false,
    NewScopeInvite {
      target_user_id: alice.id,
      requesting_user_id: bob.id,
      scope: &scope,
    },
  )
  .await
  .unwrap();

  db.insert_download_entries(vec![VersionDownloadCount {
    scope: scope.clone(),
    package: name.clone(),
    version: version.clone(),
    time_bucket: Utc::now() - chrono::Duration::hours(4),
    kind: DownloadKind::JsrMeta,
    count: 3,
  }])
  .await
  .unwrap();

  Seed {
    db,
    bob,
    alice,
    scope,
    name,
    version,
  }
}

#[tokio::test]
async fn user_and_identity_queries() {
  let Seed { db, bob, alice, .. } = seed().await;

  let user = db.get_user_by_gitlab_id(201).await.unwrap().unwrap();
  assert_eq!(user.id, bob.id);

  let user = db
    .upsert_user_by_github_id(NewUser {
      name: "Bobby",
      email: Some("bobby@example.com"),
      avatar_url: "https://example.com/bobby.png",
      github_id: Some(101),
      gitlab_id: None,
      is_blocked: false,
      is_staff: false,
    })
    .await
    .unwrap();
  assert_eq!(user.id, bob.id);
  assert_eq!(user.name, "Bobby");

  let user = db
    .upsert_user_by_gitlab_id(NewUser {
      name: "Carol",
      email: None,
      avatar_url: "https://example.com/carol.png",
      github_id: None,
      gitlab_id: Some(202),
      is_blocked: false,
      is_staff: false,
    })
    .await
    .unwrap();
  assert_eq!(user.gitlab_id, Some(202));

  let user = db.user_set_github_id(alice.id, Some(102)).await.unwrap();
  assert_eq!(user.github_id, Some(102));
  let user = db.user_set_gitlab_id(alice.id, Some(203)).await.unwrap();
  assert_eq!(user.gitlab_id, Some(203));

  let identity = db.get_github_identity(101).await.unwrap();
  assert_eq!(identity.access_token.as_deref(), Some("token"));
  let identity = db.delete_github_identity(103).await.unwrap();
  assert_eq!(identity.github_id, 103);
  let identity = db.delete_gitlab_identity(201).await;
  assert!(identity.is_err(), "bob still references this identity");

  db.upsert_service_account_token(Some("hash".to_string()))
    .await
    .unwrap();
  db.upsert_service_account_token(None).await.unwrap();

  let token = db
    .insert_token(NewToken {
      hash: "alice".to_string(),
      user_id: alice.id,
      r#type: TokenType::Personal,
      description: Some("token".to_string()),
      expires_at: None,
      permissions: None,
    })
    .await
    .unwrap();
  assert!(db.delete_token(alice.id, token.id).await.unwrap());
  assert!(!db.delete_token(alice.id, token.id).await.unwrap());

  assert_eq!(
    db.delete_user(&bob.id, true, alice.id).await.unwrap(),
    UserDeleteResult::Deleted
  );
}

#[tokio::test]
async fn package_and_version_queries() {
  let Seed {
    db,
    bob,
    scope,
    name,
    version,
    ..
  } = seed().await;

  db.insert_provenance_statement(&scope, &name, &version, "rekor")
    .await
    .unwrap();
  let package_version = db
    .get_latest_unyanked_version_for_package(&scope, &name)
    .await
    .unwrap()
    .unwrap();
  assert_eq!(package_version.rekor_log_id.as_deref(), Some("rekor"));
  assert!(package_version.meta.has_provenance);

  let package_version = db
    .get_latest_unyanked_version_for_package_with_newer_versions_count(
      &scope, &name,
    )
    .await
    .unwrap()
    .unwrap();
  assert_eq!(package_version.newer_versions_count, 0);
  let versions = db
    .list_latest_unyanked_versions_for_package(&scope, &name, 5)
    .await
    .unwrap();
  assert_eq!(versions, vec![version.clone()]);

  let package = db
    .update_package_source(&bob.id, false, &scope, &name, ReadmeSource::JSDoc)
    .await
    .unwrap();
  assert_eq!(package.readme_source, ReadmeSource::JSDoc);

  let (package, repo, _) = db
    .update_package_github_repository(
      &bob.id,
      false,
      &scope,
      &name,
      NewGithubRepository {
        id: 1,
        owner: "owner",
        name: "repo",
      },
    )
    .await
    .unwrap();
  assert_eq!(package.github_repository_id, Some(1));
  assert_eq!(repo.owner, "owner");
  let package = db
    .delete_package_github_repository(&bob.id, false, &scope, &name)
    .await
    .unwrap();
  assert_eq!(package.github_repository_id, None);

  let (newest, updated, featured) = db.package_stats().await.unwrap();
  assert_eq!(newest.len(), 1);
  assert_eq!(updated.len(), 1);
  assert_eq!(updated[0].version, version);
  assert!(featured.is_empty());

  let metrics = db.metrics().await.unwrap();
  assert_eq!(metrics.packages, 1);
  // Bob, alice, and the service account the migrations create.
  assert_eq!(metrics.users, 3);
  assert_eq!(metrics.package_versions, 1);

  let packages = db
    .get_recently_published_packages_by_user(&bob.id)
    .await
    .unwrap();
  assert_eq!(packages.len(), 1);
  assert_eq!(packages[0].0.name, name);

  let scopes = db.list_all_scopes_for_sitemap().await.unwrap();
  assert_eq!(scopes.len(), 1);
  let packages = db.list_all_packages_for_sitemap().await.unwrap();
  assert_eq!(packages.len(), 1);
}

#[tokio::test]
async fn scope_and_invite_queries() {
  let Seed {
    db,
    bob,
    alice,
    scope,
    ..
  } = seed().await;

  let (total, scopes) = db.list_scopes(0, 10, None, None).await.unwrap();
  assert_eq!(total, 1);
  let (scope_row, usage, creator) = &scopes[0];
  assert_eq!(scope_row.scope, scope);
  assert_eq!(usage.package, 1);
  assert_eq!(creator.id, bob.id);
  let (total, _) = db
    .list_scopes(0, 10, Some("sco"), Some("!creator"))
    .await
    .unwrap();
  assert_eq!(total, 1);

  let scopes = db.list_scopes_created_by_user(bob.id).await.unwrap();
  assert_eq!(scopes.len(), 1);
  let scopes = db.get_member_scopes_by_user(&bob.id).await.unwrap();
  assert_eq!(scopes.len(), 1);

  let updated = db
    .scope_set_verify_oidc_actor(&bob.id, false, &scope, true)
    .await
    .unwrap();
  assert!(updated.verify_oidc_actor);
  let updated = db
    .scope_set_description(
      &bob.id,
      false,
      &scope,
      Some("A description".to_string()),
    )
    .await
    .unwrap();
  assert_eq!(&*updated.description, "A description");

  let invites = db.get_scope_invites_by_user(&alice.id).await.unwrap();
  assert_eq!(invites.len(), 1);
  let (invite, target_user, requesting_user) = &invites[0];
  assert_eq!(invite.scope, scope);
  assert_eq!(target_user.id, alice.id);
  assert_eq!(requesting_user.id, bob.id);
  let invites = db.get_scope_invites_by_scope(&scope).await.unwrap();
  assert_eq!(invites.len(), 1);

  db.delete_scope_invite(&bob.id, false, &alice.id, &scope)
    .await
    .unwrap();
  let invites = db.get_scope_invites_by_scope(&scope).await.unwrap();
  assert!(invites.is_empty());
}

#[tokio::test]
async fn publishing_task_and_npm_tarball_queries() {
  let Seed {
    db,
    bob,
    scope,
    name,
    version,
    ..
  } = seed().await;

  let (total, tasks) =
    db.list_publishing_tasks(0, 10, None, None).await.unwrap();
  assert_eq!(total, 1);
  assert_eq!(tasks[0].1.as_ref().unwrap().id, bob.id);
  let (total, _) = db
    .list_publishing_tasks(0, 10, Some("foo"), Some("status"))
    .await
    .unwrap();
  assert_eq!(total, 1);

  let (task, user) = db
    .get_publishing_task_for_version(&scope, &name, &version)
    .await
    .unwrap();
  assert_eq!(task.package_version, version);
  assert_eq!(user.unwrap().id, bob.id);

  let tarball = db
    .get_npm_tarball(&scope, &name, &version, NPM_TARBALL_REVISION as i32)
    .await
    .unwrap()
    .unwrap();
  assert_eq!(tarball.sha512, "sha512");

  let missing = db
    .list_missing_npm_tarballs(NPM_TARBALL_REVISION as i32 + 1)
    .await
    .unwrap();
  assert_eq!(
    missing,
    vec![(scope.clone(), name.clone(), version.clone())]
  );
  let tarball = db
    .create_npm_tarball(NewNpmTarball {
      scope: &scope,
      name: &name,
      version: &version,
      revision: NPM_TARBALL_REVISION as i32 + 1,
      sha1: "sha1",
      sha512: "sha512",
      size: 1,
    })
    .await
    .unwrap();
  assert_eq!(tarball.revision, NPM_TARBALL_REVISION as i32 + 1);
  let missing = db
    .list_missing_npm_tarballs(NPM_TARBALL_REVISION as i32 + 1)
    .await
    .unwrap();
  assert!(missing.is_empty());
}

#[tokio::test]
async fn download_queries() {
  let Seed {
    db,
    scope,
    name,
    version,
    ..
  } = seed().await;

  let start = Utc::now() - chrono::Duration::days(7);
  let end = Utc::now() + chrono::Duration::days(1);
  let downloads = db
    .get_package_versions_downloads_24h(
      &scope,
      &name,
      std::slice::from_ref(&version),
      start,
      end,
    )
    .await
    .unwrap();
  assert_eq!(downloads.len(), 1);
  assert_eq!(downloads[0].count, 3);
  let downloads = db
    .get_package_downloads_24h(&scope, &name, start, end)
    .await
    .unwrap();
  assert_eq!(downloads.len(), 1);
  assert_eq!(downloads[0].count, 3);

  assert_eq!(db.cleanup_download_counts_4h(Utc::now()).await.unwrap(), 1);
  assert_eq!(db.delete_expired_oauth_states(Utc::now()).await.unwrap(), 0);
}

#[tokio::test]
async fn ticket_and_audit_log_queries() {
  let Seed { db, bob, alice, .. } = seed().await;

  let (ticket, user, _) = db
    .create_staff_outreach_ticket(
      &bob.id,
      alice.id,
      "Subject",
      serde_json::json!({}),
      "Hello",
      Some("<outreach@example.com>"),
    )
    .await
    .unwrap();
  assert_eq!(user.id, alice.id);

  let found = db
    .find_ticket_by_email_message_ids(&["<outreach@example.com>".to_string()])
    .await
    .unwrap();
  assert_eq!(found, Some(ticket.id));

  let message = db
    .ticket_add_reporter_message(
      ticket.id,
      "reporter@example.com",
      Some("Reporter"),
      Some("<reply@example.com>"),
      NewTicketMessage {
        message: "Hi".to_string(),
        internal: false,
      },
    )
    .await
    .unwrap();
  assert_eq!(
    message.author_email.as_deref(),
    Some("reporter@example.com")
  );

  let (total, audit_logs) =
    db.list_audit_logs(0, 10, None, None, false).await.unwrap();
  assert_eq!(total, audit_logs.len());
  assert!(total > 0);
  let (total, _) = db
    .list_audit_logs(0, 10, Some("scope"), Some("user"), true)
    .await
    .unwrap();
  assert!(total > 0);
}
