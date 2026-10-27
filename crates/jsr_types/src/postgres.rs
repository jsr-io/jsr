// Copyright 2024 the JSR authors. All rights reserved. MIT license.

//! `postgres-types` `FromSql`/`ToSql` impls for the id newtypes and the
//! JSON-backed model types. The Postgres enums derive theirs in `models.rs`.

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
use crate::models::ExportsMap;
use crate::models::PackageVersionMeta;
use crate::models::Permissions;
use crate::models::PublishingTaskError;
use crate::models::RuntimeCompat;

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
