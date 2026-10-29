// Copyright 2024 the JSR authors. All rights reserved. MIT license.

//! A thin layer over `tokio-postgres` that every database query goes through.
//!
//! Queries are built with [`query`], [`query_as`] or [`query_scalar`] and run
//! against an [`Executor`]: the pool (a connection per query), a pooled
//! client, or a transaction.
//!
//! Every query is sent as an unnamed statement with the types of its
//! parameters (`query_typed`), in a single round trip. Named prepared
//! statements break through Cloudflare Hyperdrive, which the API worker
//! reaches the database through. Each parameter's Postgres type comes from
//! its Rust type, see [`PgType`].
//!
//! Rows are mapped with [`FromRow`]. A model's mapper reads the columns of its
//! SQL fragment (see `sql_fragments.rs`) under an optional prefix, so the same
//! mapper serves both `SELECT {USER_SELECT}` and a join that aliases the same
//! columns as `user_*`.

use std::future::Future;

use chrono::DateTime;
use chrono::Utc;
use jsr_types::postgres::PgEnum;
use postgres_types::FromSql;
use postgres_types::Kind;
use postgres_types::ToSql;
use postgres_types::Type;
use tokio_postgres::Row;
use tokio_postgres::error::SqlState;
use uuid::Uuid;

use super::models::*;
use crate::ids::PackageName;
use crate::ids::PackagePath;
use crate::ids::ScopeDescription;
use crate::ids::ScopeName;
use crate::ids::Version;

pub type Result<T, E = Error> = std::result::Result<T, E>;

pub type Params<'a> = &'a [&'a dyn Param];

#[derive(Debug, thiserror::Error)]
pub enum Error {
  #[error("query returned no rows")]
  RowNotFound,
  #[error("column {0} not found")]
  ColumnNotFound(String),
  #[error("failed to get a database connection: {0}")]
  Pool(#[from] deadpool_postgres::PoolError),
  // `tokio_postgres::Error`'s own message is only its kind ("db error",
  // "error serializing parameter 0"), the details are in its source.
  #[error("{}", ErrorChain(.0))]
  Postgres(#[from] tokio_postgres::Error),
}

impl Error {
  pub fn code(&self) -> Option<&SqlState> {
    match self {
      Error::Postgres(err) => err.code(),
      _ => None,
    }
  }

  pub fn is_unique_violation(&self) -> bool {
    self.code() == Some(&SqlState::UNIQUE_VIOLATION)
  }

  pub fn is_foreign_key_violation(&self) -> bool {
    self.code() == Some(&SqlState::FOREIGN_KEY_VIOLATION)
  }
}

struct ErrorChain<'a>(&'a tokio_postgres::Error);

impl std::fmt::Display for ErrorChain<'_> {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    write!(f, "{}", self.0)?;
    let mut source = std::error::Error::source(self.0);
    while let Some(err) = source {
      write!(f, ": {err}")?;
      source = err.source();
    }
    Ok(())
  }
}

/// The Postgres type a query parameter is sent as.
#[derive(Clone)]
pub enum ParamType {
  Builtin(Type),
  /// A Postgres enum. Its OID differs between databases, so it is sent as an
  /// unspecified type, which Postgres infers from the query.
  Enum(&'static str),
}

impl ParamType {
  fn to_type(&self) -> Type {
    match self {
      ParamType::Builtin(ty) => ty.clone(),
      ParamType::Enum(name) => {
        Type::new(name.to_string(), 0, Kind::Pseudo, "public".to_string())
      }
    }
  }
}

/// A Rust type's Postgres type, for sending it as a query parameter.
pub trait PgType {
  const TYPE: ParamType;
}

/// A type that can be sent in an array parameter. Arrays of enums are sent
/// as `text[]`, so the query has to cast them (`$1::task_status[]`).
pub trait PgArrayElement {
  const ARRAY_TYPE: Type;
}

/// A query parameter.
pub trait Param: ToSql + Sync {
  fn param_type(&self) -> ParamType;
}

impl<T: PgType + ToSql + Sync> Param for T {
  fn param_type(&self) -> ParamType {
    T::TYPE
  }
}

macro_rules! pg_type {
  ($($ty:ty => $type:ident / $array:ident),* $(,)?) => {
    $(
      impl PgType for $ty {
        const TYPE: ParamType = ParamType::Builtin(Type::$type);
      }

      impl PgArrayElement for $ty {
        const ARRAY_TYPE: Type = Type::$array;
      }
    )*
  };
}

pg_type!(
  bool => BOOL / BOOL_ARRAY,
  i16 => INT2 / INT2_ARRAY,
  i32 => INT4 / INT4_ARRAY,
  i64 => INT8 / INT8_ARRAY,
  f32 => FLOAT4 / FLOAT4_ARRAY,
  f64 => FLOAT8 / FLOAT8_ARRAY,
  str => TEXT / TEXT_ARRAY,
  String => TEXT / TEXT_ARRAY,
  Uuid => UUID / UUID_ARRAY,
  DateTime<Utc> => TIMESTAMPTZ / TIMESTAMPTZ_ARRAY,
  serde_json::Value => JSONB / JSONB_ARRAY,
  ScopeName => TEXT / TEXT_ARRAY,
  ScopeDescription => TEXT / TEXT_ARRAY,
  PackageName => TEXT / TEXT_ARRAY,
  PackagePath => TEXT / TEXT_ARRAY,
  Version => TEXT / TEXT_ARRAY,
  PublishingTaskError => JSONB / JSONB_ARRAY,
  PackageVersionMeta => JSONB / JSONB_ARRAY,
  Permissions => JSONB / JSONB_ARRAY,
  ExportsMap => JSONB / JSONB_ARRAY,
  RuntimeCompat => JSONB / JSONB_ARRAY,
);

macro_rules! pg_enum_type {
  ($($ty:ty),* $(,)?) => {
    $(
      impl PgType for $ty {
        const TYPE: ParamType = ParamType::Enum(<$ty as PgEnum>::NAME);
      }

      impl PgArrayElement for $ty {
        const ARRAY_TYPE: Type = Type::TEXT_ARRAY;
      }
    )*
  };
}

pg_enum_type!(
  PublishingTaskStatus,
  ReadmeSource,
  TokenType,
  DependencyKind,
  DownloadKind,
  TicketKind,
  TicketStatus,
  TicketMessageDirection,
);

impl PgType for Vec<u8> {
  const TYPE: ParamType = ParamType::Builtin(Type::BYTEA);
}

impl<T: PgType + ?Sized> PgType for &T {
  const TYPE: ParamType = T::TYPE;
}

impl<T: PgType> PgType for Option<T> {
  const TYPE: ParamType = T::TYPE;
}

impl<T: PgArrayElement> PgType for Vec<T> {
  const TYPE: ParamType = ParamType::Builtin(T::ARRAY_TYPE);
}

impl<T: PgArrayElement> PgType for [T] {
  const TYPE: ParamType = ParamType::Builtin(T::ARRAY_TYPE);
}

fn typed<'a>(params: Params<'a>) -> Vec<(&'a (dyn ToSql + Sync), Type)> {
  params
    .iter()
    .map(|param| {
      let ty = param.param_type().to_type();
      (*param as &(dyn ToSql + Sync), ty)
    })
    .collect()
}

/// Something a query can run against.
pub trait Executor: Sync {
  fn query_rows(
    &self,
    sql: &str,
    params: Params<'_>,
  ) -> impl Future<Output = Result<Vec<Row>>> + Send;

  fn execute(
    &self,
    sql: &str,
    params: Params<'_>,
  ) -> impl Future<Output = Result<u64>> + Send;
}

impl Executor for deadpool_postgres::Pool {
  async fn query_rows(
    &self,
    sql: &str,
    params: Params<'_>,
  ) -> Result<Vec<Row>> {
    self.get().await?.query_rows(sql, params).await
  }

  async fn execute(&self, sql: &str, params: Params<'_>) -> Result<u64> {
    Executor::execute(&self.get().await?, sql, params).await
  }
}

macro_rules! impl_executor {
  ($ty:ty) => {
    impl Executor for $ty {
      async fn query_rows(
        &self,
        sql: &str,
        params: Params<'_>,
      ) -> Result<Vec<Row>> {
        #[cfg(target_arch = "wasm32")]
        let started = crate::worker_js::now();
        let rows = self.query_typed(sql, &typed(params)).await?;
        #[cfg(target_arch = "wasm32")]
        println!(
          "timing: db query {:.0}ms {}",
          crate::worker_js::now() - started,
          sql.split_whitespace().take(8).collect::<Vec<_>>().join(" ")
        );
        #[cfg(test)]
        assert_builtin_columns(sql, &rows);
        Ok(rows)
      }

      async fn execute(&self, sql: &str, params: Params<'_>) -> Result<u64> {
        #[cfg(target_arch = "wasm32")]
        let started = crate::worker_js::now();
        let result = self.execute_typed(sql, &typed(params)).await?;
        #[cfg(target_arch = "wasm32")]
        println!(
          "timing: db execute {:.0}ms {}",
          crate::worker_js::now() - started,
          sql.split_whitespace().take(8).collect::<Vec<_>>().join(" ")
        );
        Ok(result)
      }
    }
  };
}

/// Columns of a custom type (an enum) make tokio-postgres look the type up
/// with named prepared statements, which is what `query_typed` is avoiding.
/// Select them as `::text` instead.
#[cfg(test)]
fn assert_builtin_columns(sql: &str, rows: &[Row]) {
  if let Some(row) = rows.first() {
    for column in row.columns() {
      assert!(
        Type::from_oid(column.type_().oid()).is_some(),
        "column {} is of type {}, select it as ::text: {sql}",
        column.name(),
        column.type_(),
      );
    }
  }
}

impl_executor!(deadpool_postgres::Client);
impl_executor!(deadpool_postgres::Transaction<'_>);

/// A query returning raw rows. See [`query_as`] and [`query_scalar`] for
/// queries returning models and single values.
///
/// The SQL is `'static` so that every statement is fixed at compile time:
/// values go in parameters, and shared SQL is composed with `concatcp!` (see
/// `sort_by!` for picking between statements at runtime).
pub fn query<'a>(sql: &'static str, params: Params<'a>) -> Query<'a> {
  Query { sql, params }
}

/// A query whose rows are mapped with `T`'s [`FromRow`] impl.
pub fn query_as<'a, T: FromRow>(
  sql: &'static str,
  params: Params<'a>,
) -> Map<'a, impl Fn(&Row) -> Result<T>> {
  query(sql, params).map(T::from_row)
}

/// A query returning the first column of each row.
pub fn query_scalar<'a, T: for<'r> FromSql<'r>>(
  sql: &'static str,
  params: Params<'a>,
) -> Map<'a, impl Fn(&Row) -> Result<T>> {
  query(sql, params).map(|row| Ok(row.try_get(0)?))
}

pub struct Query<'a> {
  sql: &'static str,
  params: Params<'a>,
}

impl<'a> Query<'a> {
  pub fn map<T, F: Fn(&Row) -> Result<T>>(self, f: F) -> Map<'a, F> {
    Map { query: self, f }
  }

  pub async fn execute(self, executor: &impl Executor) -> Result<u64> {
    executor.execute(self.sql, self.params).await
  }

  pub async fn fetch_all(self, executor: &impl Executor) -> Result<Vec<Row>> {
    executor.query_rows(self.sql, self.params).await
  }

  /// The first row, ignoring the rest. Fails with [`Error::RowNotFound`] if
  /// there is none.
  pub async fn fetch_one(self, executor: &impl Executor) -> Result<Row> {
    self
      .fetch_optional(executor)
      .await?
      .ok_or(Error::RowNotFound)
  }

  /// The first row, if any, ignoring the rest.
  pub async fn fetch_optional(
    self,
    executor: &impl Executor,
  ) -> Result<Option<Row>> {
    Ok(self.fetch_all(executor).await?.into_iter().next())
  }
}

pub struct Map<'a, F> {
  query: Query<'a>,
  f: F,
}

impl<T, F: Fn(&Row) -> Result<T>> Map<'_, F> {
  pub async fn fetch_all(self, executor: &impl Executor) -> Result<Vec<T>> {
    let rows = self.query.fetch_all(executor).await?;
    rows.iter().map(&self.f).collect()
  }

  pub async fn fetch_one(self, executor: &impl Executor) -> Result<T> {
    (self.f)(&self.query.fetch_one(executor).await?)
  }

  pub async fn fetch_optional(
    self,
    executor: &impl Executor,
  ) -> Result<Option<T>> {
    let row = self.query.fetch_optional(executor).await?;
    row.as_ref().map(&self.f).transpose()
  }
}

/// Maps a row to a model.
pub trait FromRow: Sized {
  /// Reads the model from the columns named `{prefix}{column}`.
  fn from_columns(columns: Columns<'_>) -> Result<Self>;

  fn from_row(row: &Row) -> Result<Self> {
    Self::from_columns(Columns::new(row, ""))
  }

  fn from_row_prefixed(row: &Row, prefix: &str) -> Result<Self> {
    Self::from_columns(Columns::new(row, prefix))
  }

  /// For a LEFT JOIN: `None` if the `{prefix}{key}` column is NULL.
  fn from_row_optional(
    row: &Row,
    prefix: &str,
    key: &str,
  ) -> Result<Option<Self>> {
    let columns = Columns::new(row, prefix);
    if columns.is_null(key)? {
      return Ok(None);
    }
    Self::from_columns(columns).map(Some)
  }
}

/// The columns of a row that share a prefix, such as the `user_*` columns of
/// a join.
#[derive(Clone, Copy)]
pub struct Columns<'a> {
  row: &'a Row,
  prefix: &'a str,
}

impl<'a> Columns<'a> {
  pub fn new(row: &'a Row, prefix: &'a str) -> Self {
    Self { row, prefix }
  }

  fn index(&self, column: &str) -> Result<usize> {
    self
      .row
      .columns()
      .iter()
      .position(|c| c.name().strip_prefix(self.prefix) == Some(column))
      .ok_or_else(|| Error::ColumnNotFound(format!("{}{column}", self.prefix)))
  }

  pub fn get<T: FromSql<'a>>(&self, column: &str) -> Result<T> {
    Ok(self.row.try_get(self.index(column)?)?)
  }

  /// Whether the column is NULL, whatever its type. Used to tell a missing
  /// LEFT JOIN row apart from a present one.
  pub fn is_null(&self, column: &str) -> Result<bool> {
    struct AnyValue;

    impl<'a> FromSql<'a> for AnyValue {
      fn from_sql(
        _: &Type,
        _: &'a [u8],
      ) -> Result<Self, Box<dyn std::error::Error + Sync + Send>> {
        Ok(AnyValue)
      }

      fn accepts(_: &Type) -> bool {
        true
      }
    }

    Ok(self.get::<Option<AnyValue>>(column)?.is_none())
  }
}
