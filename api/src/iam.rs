// Copyright 2024 the JSR authors. All rights reserved. MIT license.
use hyper::Body;
use hyper::Request;
use routerify::prelude::RequestExt;
use uuid::Uuid;

use crate::api::ApiError;
use crate::db::Database;
use crate::db::PackagePublishPermission;
use crate::db::Permission;
use crate::db::Permissions;
use crate::db::Token;
use crate::db::TokenType;
use crate::db::User;
use crate::ids::PackageName;
use crate::ids::ScopeName;
use crate::ids::Version;
use crate::util::GithubOidcTokenAud;

pub struct IamHandler<'s> {
  db: &'s Database,
  principal: Principal,
  permissions: Option<Permissions>,
  interactive: bool,
  sudo: bool,
}

impl<'s> IamHandler<'s> {
  pub fn is_anonymous(&self) -> bool {
    matches!(self.principal, Principal::Anonymous)
  }

  pub async fn check_scope_write_access(
    &self,
    scope: &ScopeName,
  ) -> Result<(), ApiError> {
    if self.permissions.is_some() {
      // There is no specific permission that allows scope write access, so if
      // the permissions are restricted, this action is also restricted.
      return Err(ApiError::MissingPermission);
    }

    match &self.principal {
      Principal::User(user) if user.is_staff && self.sudo => Ok(()),
      Principal::User(user) => {
        self
          .db
          .get_scope_member(scope, user.id)
          .await?
          .ok_or(ApiError::ActorNotScopeMember)?;
        Ok(())
      }
      Principal::GitHubActions { .. } => Err(ApiError::ActorNotAuthorized),
      Principal::Anonymous => Err(ApiError::MissingAuthentication),
    }
  }

  pub async fn check_scope_admin_access(
    &self,
    scope: &ScopeName,
  ) -> Result<(), ApiError> {
    if self.permissions.is_some() {
      // There is no specific permission that allows scope admin access, so if
      // the permissions are restricted, this action is also restricted.
      return Err(ApiError::MissingPermission);
    }

    match &self.principal {
      Principal::User(user) if user.is_staff && self.sudo => Ok(()),
      Principal::User(user) => {
        let scope_member = self
          .db
          .get_scope_member(scope, user.id)
          .await?
          .ok_or(ApiError::ActorNotScopeMember)?;
        if !scope_member.is_admin {
          return Err(ApiError::ActorNotScopeAdmin);
        }
        Ok(())
      }
      Principal::GitHubActions { .. } => Err(ApiError::ActorNotAuthorized),
      Principal::Anonymous => Err(ApiError::MissingAuthentication),
    }
  }

  pub async fn check_scope_member_delete_access(
    &self,
    scope: &ScopeName,
    member_id: Uuid,
  ) -> Result<(), ApiError> {
    if self.permissions.is_some() {
      // There is no specific permission that allows scope admin access, so if
      // the permissions are restricted, this action is also restricted.
      return Err(ApiError::MissingPermission);
    }

    match &self.principal {
      Principal::User(user) if user.is_staff && self.sudo => Ok(()),
      Principal::User(user) => {
        let scope_member = self
          .db
          .get_scope_member(scope, user.id)
          .await?
          .ok_or(ApiError::ActorNotScopeMember)?;
        if user.id != member_id && !scope_member.is_admin {
          return Err(ApiError::ActorNotScopeAdmin);
        }
        Ok(())
      }
      Principal::GitHubActions { .. } => Err(ApiError::ActorNotAuthorized),
      Principal::Anonymous => Err(ApiError::MissingAuthentication),
    }
  }

  pub async fn check_publish_access(
    &self,
    scope_: &ScopeName,
    package_: &PackageName,
    version_: &Version,
  ) -> Result<(PublishAccessRestriction, Option<Uuid>), ApiError> {
    let access_restriction = if let Some(permissions) = &self.permissions {
      let access_restriction =
        permissions
          .0
          .iter()
          .find_map(|permission| match permission {
            Permission::PackagePublish(PackagePublishPermission::Version {
              scope,
              package,
              version,
              tarball_hash,
            }) if scope == scope_
              && package == package_
              && version == version_ =>
            {
              Some(PublishAccessRestriction {
                tarball_hash: Some(tarball_hash.clone()),
              })
            }
            Permission::PackagePublish(PackagePublishPermission::Package {
              scope,
              package,
            }) if scope == scope_ && package == package_ => {
              Some(PublishAccessRestriction { tarball_hash: None })
            }
            Permission::PackagePublish(PackagePublishPermission::Scope {
              scope,
            }) if scope == scope_ => {
              Some(PublishAccessRestriction { tarball_hash: None })
            }
            _ => None,
          });
      access_restriction.ok_or(ApiError::MissingPermission)?
    } else {
      PublishAccessRestriction { tarball_hash: None }
    };
    match &self.principal {
      Principal::User(user) if user.is_staff && self.sudo => {
        Ok((access_restriction, Some(user.id)))
      }
      Principal::User(user) => {
        let scope = self
          .db
          .get_scope(scope_)
          .await?
          .ok_or(ApiError::ScopeNotFound)?;
        if scope.require_publishing_from_ci {
          return Err(ApiError::ScopeRequiresPublishingFromCI);
        }
        self
          .db
          .get_scope_member(scope_, user.id)
          .await?
          .ok_or(ApiError::ActorNotScopeMember)?;
        Ok((access_restriction, Some(user.id)))
      }
      Principal::GitHubActions { repo_id, user } => {
        let scope = self
          .db
          .get_scope(scope_)
          .await?
          .ok_or(ApiError::ScopeNotFound)?;
        if scope.verify_oidc_actor {
          let user = user.as_ref().ok_or(ApiError::ActorNotScopeMember)?;
          self
            .db
            .get_scope_member(scope_, user.id)
            .await?
            .ok_or(ApiError::ActorNotScopeMember)?;
        }
        let (package, _, _) = self
          .db
          .get_package(scope_, package_)
          .await?
          .ok_or(ApiError::PackageNotFound)?;
        if package.github_repository_id != Some(*repo_id) {
          return Err(ApiError::ActorNotAuthorized);
        }
        Ok((access_restriction, user.as_ref().map(|user| user.id)))
      }
      Principal::Anonymous => Err(ApiError::MissingAuthentication),
    }
  }

  pub fn check_current_user_access(&self) -> Result<&User, ApiError> {
    if self.permissions.is_some() {
      // There is no specific permission that allows access to current user, so
      // if the permissions are restricted, this action is also restricted.
      return Err(ApiError::MissingPermission);
    }
    match &self.principal {
      Principal::User(user) => Ok(user),
      Principal::GitHubActions { .. } => Err(ApiError::ActorNotUser),
      Principal::Anonymous => Err(ApiError::MissingAuthentication),
    }
  }

  pub fn check_authorization_approve_access(&self) -> Result<&User, ApiError> {
    if self.permissions.is_some() {
      // There is no specific permission that allows authorization approve
      // access, so if the permissions are restricted, this action is also
      // restricted.
      return Err(ApiError::MissingPermission);
    }
    match &self.principal {
      Principal::User(user) if self.interactive => Ok(user),
      Principal::User(_) => Err(ApiError::CredentialNotInteractive),
      Principal::GitHubActions { .. } => Err(ApiError::ActorNotUser),
      Principal::Anonymous => Err(ApiError::MissingAuthentication),
    }
  }

  pub fn check_admin_access(&self) -> Result<(), ApiError> {
    match &self.principal {
      Principal::User(user) if user.is_staff => Ok(()),
      Principal::User(_) => Err(ApiError::ActorNotAuthorized),
      Principal::GitHubActions { .. } => Err(ApiError::ActorNotAuthorized),
      Principal::Anonymous => Err(ApiError::MissingAuthentication),
    }
  }
}

pub struct PublishAccessRestriction {
  pub tarball_hash: Option<String>,
}

#[derive(Clone)]
pub enum Principal {
  User(User),
  GitHubActions { repo_id: i64, user: Option<User> },
  Anonymous,
}

#[derive(Clone)]
pub struct IamInfo {
  /// The principal that is authenticated for this request. This determines
  /// which resources the request is allowed to access, and what actions it is
  /// allowed to perform on those resources.
  pub principal: Principal,
  /// Permissions attached to the request, which limit the scope of the action
  /// that is allowed. Permissions never expand the scope beyond what the
  /// principal is allowed to do.
  pub permissions: Option<Permissions>,
  /// Whether the request comes from an interactive system (web portal), or via
  /// an automated system (GitHub Actions / cli with device token).
  pub interactive: bool,
  /// Whether the request is being made with sudo privileges, which allows
  /// staff users to bypass some access restrictions.
  pub sudo: bool,
}

impl IamInfo {
  pub fn anonymous() -> Self {
    IamInfo {
      principal: Principal::Anonymous,
      permissions: None,
      interactive: false,
      sudo: false,
    }
  }
}

impl From<(Token, User, bool)> for IamInfo {
  fn from((token, user, sudo): (Token, User, bool)) -> Self {
    assert_eq!(token.user_id, user.id);
    IamInfo {
      principal: Principal::User(user),
      permissions: token.permissions,
      interactive: token.r#type == TokenType::Web,
      sudo,
    }
  }
}

impl From<(i64, GithubOidcTokenAud, Option<User>)> for IamInfo {
  fn from(
    (repo_id, aud, user): (i64, GithubOidcTokenAud, Option<User>),
  ) -> Self {
    IamInfo {
      principal: Principal::GitHubActions { repo_id, user },
      permissions: Some(aud.permissions),
      interactive: false,
      sudo: false,
    }
  }
}

pub trait ReqIamExt {
  fn iam(&self) -> IamHandler;
}

impl ReqIamExt for Request<Body> {
  fn iam(&self) -> IamHandler {
    let db = self.data().unwrap();
    let IamInfo {
      principal,
      permissions,
      interactive,
      sudo,
    } = self.context().unwrap();
    IamHandler {
      db,
      principal,
      permissions,
      interactive,
      sudo,
    }
  }
}
