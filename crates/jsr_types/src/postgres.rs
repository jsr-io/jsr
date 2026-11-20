// Copyright 2024 the JSR authors. All rights reserved. MIT license.

//! `postgres-types` `FromSql`/`ToSql` impls for the id newtypes, the
//! JSON-backed model types and the Postgres enums.

use bytes::BytesMut;
use postgres_types::FromSql;
use postgres_types::IsNull;
use postgres_types::Json;
use postgres_types::ToSql;
use postgres_types::Type;
use postgres_types::accepts;
use postgres_types::to_sql_checked;
use std::error::Error;

use crate::ids::PackageName;
use crate::ids::PackagePath;
use crate::ids::ScopeDescription;
use crate::ids::ScopeName;
use crate::ids::Version;
use crate::models::DependencyKind;
use crate::models::DownloadKind;
use crate::models::ExportsMap;
use crate::models::PackageVersionMeta;
use crate::models::Permissions;
use crate::models::PublishingTaskError;
use crate::models::PublishingTaskStatus;
use crate::models::ReadmeSource;
use crate::models::RuntimeCompat;
use crate::models::TicketKind;
use crate::models::TicketMessageDirection;
use crate::models::TicketStatus;
use crate::models::TokenType;

type BoxError = Box<dyn Error + Sync + Send>;

/// Stores a validated string newtype as `text`, re-validating on the way out
/// of the database.
macro_rules! text_newtype {
  ($ty:ty, $parse:expr) => {
    impl<'a> FromSql<'a> for $ty {
      fn from_sql(ty: &Type, raw: &'a [u8]) -> Result<Self, BoxError> {
        let s = <String as FromSql>::from_sql(ty, raw)?;
        $parse(s).map_err(|e| Box::new(e) as BoxError)
      }

      accepts!(TEXT, VARCHAR);
    }

    impl ToSql for $ty {
      fn to_sql(
        &self,
        ty: &Type,
        out: &mut BytesMut,
      ) -> Result<IsNull, BoxError> {
        let s: &str = self;
        s.to_sql(ty, out)
      }

      accepts!(TEXT, VARCHAR);
      to_sql_checked!();
    }
  };
}

text_newtype!(ScopeName, ScopeName::new);
text_newtype!(ScopeDescription, ScopeDescription::new);
text_newtype!(PackageName, PackageName::new);
text_newtype!(PackagePath, PackagePath::new);

impl<'a> FromSql<'a> for Version {
  fn from_sql(ty: &Type, raw: &'a [u8]) -> Result<Self, BoxError> {
    let s = <&str as FromSql>::from_sql(ty, raw)?;
    Version::new(s).map_err(|e| Box::new(e) as BoxError)
  }

  accepts!(TEXT, VARCHAR);
}

impl ToSql for Version {
  fn to_sql(&self, ty: &Type, out: &mut BytesMut) -> Result<IsNull, BoxError> {
    self.to_string().to_sql(ty, out)
  }

  accepts!(TEXT, VARCHAR);
  to_sql_checked!();
}

/// Stores a serde type as `json`/`jsonb`.
macro_rules! json_type {
  ($ty:ty) => {
    impl<'a> FromSql<'a> for $ty {
      fn from_sql(ty: &Type, raw: &'a [u8]) -> Result<Self, BoxError> {
        Json::<$ty>::from_sql(ty, raw).map(|json| json.0)
      }

      accepts!(JSON, JSONB);
    }

    impl ToSql for $ty {
      fn to_sql(
        &self,
        ty: &Type,
        out: &mut BytesMut,
      ) -> Result<IsNull, BoxError> {
        Json(self).to_sql(ty, out)
      }

      accepts!(JSON, JSONB);
      to_sql_checked!();
    }
  };
}

json_type!(PublishingTaskError);
json_type!(Permissions);
json_type!(RuntimeCompat);

impl<'a> FromSql<'a> for PackageVersionMeta {
  fn from_sql(ty: &Type, raw: &'a [u8]) -> Result<Self, BoxError> {
    Json::<PackageVersionMeta>::from_sql(ty, raw).map(|json| json.0)
  }

  // Versions published before the column was backfilled have no meta.
  fn from_sql_null(_: &Type) -> Result<Self, BoxError> {
    Ok(Self::default())
  }

  accepts!(JSON, JSONB);
}

impl ToSql for PackageVersionMeta {
  fn to_sql(&self, ty: &Type, out: &mut BytesMut) -> Result<IsNull, BoxError> {
    Json(self).to_sql(ty, out)
  }

  accepts!(JSON, JSONB);
  to_sql_checked!();
}

impl<'a> FromSql<'a> for ExportsMap {
  fn from_sql(ty: &Type, raw: &'a [u8]) -> Result<Self, BoxError> {
    Json::from_sql(ty, raw).map(|json| ExportsMap::new(json.0))
  }

  accepts!(JSON, JSONB);
}

impl ToSql for ExportsMap {
  fn to_sql(&self, ty: &Type, out: &mut BytesMut) -> Result<IsNull, BoxError> {
    Json(self.as_inner()).to_sql(ty, out)
  }

  accepts!(JSON, JSONB);
  to_sql_checked!();
}

/// A Rust enum stored as a Postgres enum.
pub trait PgEnum: Sized + 'static {
  /// The Postgres type's name.
  const NAME: &'static str;
  /// Every variant with its Postgres label.
  const VARIANTS: &'static [(Self, &'static str)];
}

/// Maps a Rust enum to a Postgres enum by label.
///
/// Labels are also accepted as and written to `text`: queries select enum
/// columns as `::text`, so the client never has to look up the enum's type,
/// and send enum arrays as `text[]`.
macro_rules! pg_enum {
  ($ty:ident, $name:literal, { $($variant:ident => $label:literal),+ $(,)? }) => {
    impl PgEnum for $ty {
      const NAME: &'static str = $name;
      const VARIANTS: &'static [(Self, &'static str)] =
        &[$(($ty::$variant, $label)),+];
    }

    impl<'a> FromSql<'a> for $ty {
      fn from_sql(_: &Type, raw: &'a [u8]) -> Result<Self, BoxError> {
        match std::str::from_utf8(raw)? {
          $($label => Ok($ty::$variant),)+
          label => Err(format!("unknown {} label {label:?}", $name).into()),
        }
      }

      fn accepts(ty: &Type) -> bool {
        ty.name() == $name || matches!(*ty, Type::TEXT | Type::VARCHAR)
      }
    }

    impl ToSql for $ty {
      fn to_sql(&self, _: &Type, out: &mut BytesMut) -> Result<IsNull, BoxError> {
        let label = match self {
          $($ty::$variant => $label,)+
        };
        out.extend_from_slice(label.as_bytes());
        Ok(IsNull::No)
      }

      fn accepts(ty: &Type) -> bool {
        ty.name() == $name || matches!(*ty, Type::TEXT | Type::VARCHAR)
      }

      to_sql_checked!();
    }
  };
}

pg_enum!(PublishingTaskStatus, "task_status", {
  Pending => "pending",
  Processing => "processing",
  Processed => "processed",
  Success => "success",
  Failure => "failure",
});
pg_enum!(ReadmeSource, "package_readme_source", {
  Readme => "readme",
  JSDoc => "jsdoc",
});
pg_enum!(TokenType, "token_type", {
  Web => "web",
  Device => "device",
  Personal => "personal",
});
pg_enum!(DependencyKind, "dependency_kind", {
  Jsr => "jsr",
  Npm => "npm",
});
pg_enum!(DownloadKind, "download_kind", {
  JsrMeta => "jsr_meta",
  NpmTgz => "npm_tgz",
});
pg_enum!(TicketKind, "ticket_kind", {
  UserScopeQuotaIncrease => "user_scope_quota_increase",
  ScopeQuotaIncrease => "scope_quota_increase",
  ScopeClaim => "scope_claim",
  PackageReport => "package_report",
  Other => "other",
  StaffOutreach => "staff_outreach",
});
pg_enum!(TicketStatus, "ticket_status", {
  Open => "open",
  WaitingOnUser => "waiting_on_user",
  WaitingOnSupport => "waiting_on_support",
  Closed => "closed",
  Spam => "spam",
});
pg_enum!(TicketMessageDirection, "ticket_message_direction", {
  Inbound => "inbound",
  Outbound => "outbound",
});
