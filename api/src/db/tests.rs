// Copyright 2024 the JSR authors. All rights reserved. MIT license.
use crate::db::*;
use crate::ids::PackageName;
use crate::ids::PackagePath;
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

  let _scope = db.create_scope(&scope_name, user_id).await.unwrap();
  let res = db.create_package(&scope_name, &package_name).await.unwrap();
  assert!(matches!(res, CreatePackageResult::Ok(_)));

  let CreatePublishingTaskResult::Created(pt) = db
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
  let CreatePublishingTaskResult::Exists(pt_reused) = res else {
    unreachable!() // does conflict with existing task
  };
  assert_eq!(pt_reused.id, pt.id);

  let pt2: PublishingTask =
    db.get_publishing_task(pt.id).await.unwrap().unwrap();
  assert_eq!(pt2.id, pt2.id);
  assert_eq!(pt2.package_scope, scope_name);
  assert_eq!(pt2.package_name, package_name);
  assert_eq!(pt2.status, PublishingTaskStatus::Pending);
  assert!(pt.error.is_none());

  let no_pt = db.get_publishing_task(uuid::Uuid::new_v4()).await.unwrap();
  assert!(no_pt.is_none());

  let pt3 = db
    .update_publishing_task_status(
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

  let CreatePublishingTaskResult::Created(pt4) = db
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
    pt4.id,
    PublishingTaskStatus::Pending,
    PublishingTaskStatus::Success,
    None,
  )
  .await
  .unwrap();

  let pt5 = db.get_publishing_task(pt4.id).await.unwrap().unwrap();
  assert_eq!(pt5.status, PublishingTaskStatus::Success);
  assert!(pt5.updated_at > pt5.created_at);
}

#[tokio::test]
async fn users() {
  let db = EphemeralDatabase::create().await;

  let new_user = NewUser {
    name: "Alice",
    email: Some("alice@example.com"),
    avatar_url: "https://example.com/alice.png",
    github_id: None,
    is_blocked: false,
    is_staff: true,
  };
  let user = db.insert_user(new_user).await.unwrap();
  assert_eq!(user.name, "Alice");
  assert_eq!(user.avatar_url, "https://example.com/alice.png");
  assert_eq!(user.email, Some("alice@example.com".to_string()));
  assert_eq!(user.scope_limit, 3);
  assert!(user.is_staff);
  assert!(!user.is_blocked);

  let user_ = db.user_set_staff(user.id, false).await.unwrap();
  assert!(!user_.is_staff);

  let user_ = db.user_set_staff(user.id, true).await.unwrap();
  assert!(user_.is_staff);

  let user_ = db.user_set_blocked(user.id, true).await.unwrap();
  assert!(user_.is_blocked);

  let user_ = db.user_set_blocked(user.id, false).await.unwrap();
  assert!(!user_.is_blocked);

  let user2 = db.get_user(user.id).await.unwrap().unwrap();
  assert_eq!(user2.id, user.id);
  assert_eq!(user2.name, "Alice");
  assert_eq!(user2.avatar_url, "https://example.com/alice.png");
  assert_eq!(user2.email, Some("alice@example.com".to_string()));

  let no_user = db.get_user(uuid::Uuid::new_v4()).await.unwrap();
  assert!(no_user.is_none());

  let (total_users, users) = db.list_users(0, 20, None).await.unwrap();
  assert_eq!(total_users, 2);
  assert_eq!(users.len(), 2);
  assert_eq!(users[0].id, user.id);
  assert_eq!(users[0].name, "Alice");
  assert_eq!(users[0].avatar_url, "https://example.com/alice.png");
  assert_eq!(users[0].email, Some("alice@example.com".to_string()));
  assert_eq!(users[0].scope_usage, 0);
  assert_eq!(users[1].id, uuid::Uuid::default()); // added by migrations

  let user3 = db.delete_user(user.id).await.unwrap().unwrap();
  assert_eq!(user3.id, user.id);

  let no_user = db.get_user(user.id).await.unwrap();
  assert!(no_user.is_none());

  let (total_users, users) = db.list_users(0, 20, None).await.unwrap();
  assert_eq!(total_users, 1);
  assert_eq!(users.len(), 1); // just the default user added by migrations

  let no_user = db.delete_user(user.id).await.unwrap();
  assert!(no_user.is_none());
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
      is_blocked: false,
      is_staff: false,
    })
    .await
    .unwrap();

  let scope_name = "scope".try_into().unwrap();
  let package_name = "testpkg".try_into().unwrap();

  db.create_scope(&scope_name, alice.id).await.unwrap();

  let alice2 = db.get_user(alice.id).await.unwrap().unwrap();
  assert_eq!(alice2.scope_usage, 1);

  assert!(db
    .get_scope_member(&scope_name, alice.id)
    .await
    .unwrap()
    .is_some());

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
    .list_packages_by_scope(&scope_name, 0, 100)
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
      is_blocked: false,
      is_staff: false,
    })
    .await
    .unwrap();

  let scope_name = "scope".try_into().unwrap();

  db.create_scope(&scope_name, bob.id).await.unwrap();

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
      is_blocked: false,
      is_staff: false,
      avatar_url: "https://example.com/bob.png",
    })
    .await
    .unwrap();

  db.create_scope(&scope, bob.id).await.unwrap();

  let CreatePackageResult::Ok(_package) =
    db.create_package(&scope, &package_name).await.unwrap()
  else {
    unreachable!()
  };

  let version = Version::try_from("1.2.3").unwrap();
  let config_file = PackagePath::try_from("/jsr.json").unwrap();
  let CreatePublishingTaskResult::Created(task) = db
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
async fn package_files() {
  let db = EphemeralDatabase::create().await;

  let user = db
    .insert_user(NewUser {
      name: "Alice",
      email: None,
      avatar_url: "https://example.com/alice.png",
      github_id: None,
      is_blocked: false,
      is_staff: false,
    })
    .await
    .unwrap();

  let scope_name = "scope".try_into().unwrap();
  let package_name = "testpkg".try_into().unwrap();
  let version = "1.2.3".try_into().unwrap();

  db.create_scope(&scope_name, user.id).await.unwrap();

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
  };
  let oauth_state = db.insert_oauth_state(new_oauth_state).await.unwrap();
  assert_eq!(oauth_state.csrf_token, "a");
  assert_eq!(oauth_state.pkce_code_verifier, "b");
  assert_eq!(oauth_state.redirect_url, "c");

  let oauth_state2 = db
    .get_oauth_state(&oauth_state.csrf_token)
    .await
    .unwrap()
    .unwrap();
  assert_eq!(oauth_state2.csrf_token, "a");
  assert_eq!(oauth_state2.pkce_code_verifier, "b");
  assert_eq!(oauth_state2.redirect_url, "c");

  let oauth_state3 = db
    .delete_oauth_state(&oauth_state.csrf_token)
    .await
    .unwrap()
    .unwrap();
  assert_eq!(oauth_state3.csrf_token, "a");
  assert_eq!(oauth_state3.pkce_code_verifier, "b");
  assert_eq!(oauth_state3.redirect_url, "c");

  assert!(db
    .delete_oauth_state(&oauth_state.csrf_token)
    .await
    .unwrap()
    .is_none())
}

#[tokio::test]
async fn tokens() {
  let db = EphemeralDatabase::create().await;

  let new_user = NewUser {
    name: "Alice",
    email: Some("alice@example.com"),
    avatar_url: "https://example.com/alice.png",
    github_id: None,
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

#[test]
fn alias_target_parsing() {
  use std::str::FromStr;
  assert_eq!(
    AliasTarget::from_str("npm:express").unwrap(),
    AliasTarget::Npm("express".to_string())
  );
  assert_eq!(
    AliasTarget::from_str("jsr:@ry/mysql").unwrap(),
    AliasTarget::Jsr(
      ScopeName::new("ry".to_string()).unwrap(),
      PackageName::new("mysql".to_string()).unwrap()
    )
  );
  assert!(AliasTarget::from_str("bad").is_err());
}

#[tokio::test]
async fn aliases() {
  let db = EphemeralDatabase::create().await;

  let aliases = db.list_aliases_for_package("express").await.unwrap();
  assert_eq!(aliases.len(), 0);

  let alias = db
    .create_alias("express", 1, AliasTarget::Npm("express".to_string()))
    .await
    .unwrap();
  assert_eq!(alias.name, "express");
  assert_eq!(alias.major_version, 1);
  assert_eq!(alias.target, AliasTarget::Npm("express".to_string()));

  let aliases = db.list_aliases_for_package("express").await.unwrap();
  assert_eq!(aliases.len(), 1);
  assert_eq!(aliases[0].target, AliasTarget::Npm("express".to_string()));

  // Because there's no such package @badscope/badpackage, this should fail the
  // foreign key constraint.
  let err = db
    .create_alias(
      "foo",
      1,
      AliasTarget::Jsr(
        "badscope".try_into().unwrap(),
        "badpackage".try_into().unwrap(),
      ),
    )
    .await
    .unwrap_err();
  assert_eq!(
    err.as_database_error().unwrap().code().unwrap(),
    "23503",
    "expected 'violates foreign key constraint'"
  );
}
