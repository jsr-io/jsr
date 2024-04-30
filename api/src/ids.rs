use std::borrow::Cow;

// Copyright 2024 the JSR authors. All rights reserved. MIT license.
use deno_semver::VersionParseError;
use sqlx::postgres::PgValueRef;
use sqlx::Postgres;
use thiserror::Error;

/// A scope name, like `user` or `admin`. The name is not prefixed with an @.
/// The name must be at least 2 characters long, and at most 20 characters long.
/// The name must only contain alphanumeric characters and hyphens.
/// The name must not start or end with a hyphen.
#[derive(Clone, PartialEq, Eq, Hash)]
pub struct ScopeName(String);

impl ScopeName {
  pub fn new(name: String) -> Result<Self, ScopeNameValidateError> {
    if name.len() < 2 {
      return Err(ScopeNameValidateError::TooShort);
    }

    if name.len() > 20 {
      return Err(ScopeNameValidateError::TooLong);
    }

    if !name
      .chars()
      // temp allow underscores
      .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-')
    {
      return Err(ScopeNameValidateError::InvalidCharacters);
    }

    if name.starts_with('-') || name.ends_with('-') {
      return Err(ScopeNameValidateError::LeadingOrTrailingHyphens);
    }

    if name.contains("--") {
      return Err(ScopeNameValidateError::DoubleHyphens);
    }

    Ok(ScopeName(name))
  }
}

impl TryFrom<&str> for ScopeName {
  type Error = ScopeNameValidateError;
  fn try_from(value: &str) -> Result<Self, Self::Error> {
    Self::new(value.to_owned())
  }
}

impl TryFrom<String> for ScopeName {
  type Error = ScopeNameValidateError;
  fn try_from(value: String) -> Result<Self, Self::Error> {
    Self::new(value)
  }
}

impl std::fmt::Display for ScopeName {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    write!(f, "{}", self.0)
  }
}

impl std::fmt::Debug for ScopeName {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    write!(f, "{}", self.0)
  }
}

impl<'a> serde::Deserialize<'a> for ScopeName {
  fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
  where
    D: serde::Deserializer<'a>,
  {
    let s: String = String::deserialize(deserializer)?;
    Self::new(s).map_err(serde::de::Error::custom)
  }
}

impl serde::Serialize for ScopeName {
  fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
  where
    S: serde::Serializer,
  {
    self.0.serialize(serializer)
  }
}

impl sqlx::Decode<'_, Postgres> for ScopeName {
  fn decode(
    value: PgValueRef<'_>,
  ) -> Result<Self, Box<dyn std::error::Error + 'static + Send + Sync>> {
    let s: String = sqlx::Decode::<'_, Postgres>::decode(value)?;
    Self::new(s).map_err(|e| Box::new(e) as _)
  }
}

impl<'q> sqlx::Encode<'q, Postgres> for ScopeName {
  fn encode_by_ref(
    &self,
    buf: &mut <Postgres as sqlx::database::HasArguments<'q>>::ArgumentBuffer,
  ) -> sqlx::encode::IsNull {
    <String as sqlx::Encode<'_, Postgres>>::encode_by_ref(&self.0, buf)
  }
}

impl sqlx::Type<Postgres> for ScopeName {
  fn type_info() -> <Postgres as sqlx::Database>::TypeInfo {
    <String as sqlx::Type<Postgres>>::type_info()
  }
}

impl std::ops::Deref for ScopeName {
  type Target = String;

  fn deref(&self) -> &Self::Target {
    &self.0
  }
}

#[derive(Debug, Clone, Error)]
pub enum ScopeNameValidateError {
  #[error("scope name must be at least 2 characters long")]
  TooShort,

  #[error("scope name must be at most 20 characters long")]
  TooLong,

  #[error("scope name must contain only lowercase ascii alphanumeric characters and hyphens")]
  InvalidCharacters,

  #[error("scope name must not start or end with a hyphen")]
  LeadingOrTrailingHyphens,

  #[error("scope name must not contain double hyphens")]
  DoubleHyphens,
}

/// A package name, like 'foo' or 'bar'. The name is not prefixed with an @.
/// The name must be at least 2 character long, and at most 32 characters long.
/// The name must only contain alphanumeric characters and hyphens.
/// The name must not start or end with a hyphen.
#[derive(Clone, PartialEq, Eq, Hash)]
pub struct PackageName(String);

impl PackageName {
  pub fn new(name: String) -> Result<Self, PackageNameValidateError> {
    if name.len() < 2 {
      return Err(PackageNameValidateError::TooShort);
    }

    if name.len() > 32 {
      return Err(PackageNameValidateError::TooLong);
    }

    if !name
      .chars()
      .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-')
    {
      return Err(PackageNameValidateError::InvalidCharacters);
    }

    if name.starts_with('-') || name.ends_with('-') {
      return Err(PackageNameValidateError::LeadingOrTrailingHyphens);
    }

    if name.contains("--") {
      return Err(PackageNameValidateError::DoubleHyphens);
    }

    Ok(PackageName(name))
  }
}

impl TryFrom<&str> for PackageName {
  type Error = PackageNameValidateError;
  fn try_from(value: &str) -> Result<Self, Self::Error> {
    Self::new(value.to_owned())
  }
}

impl TryFrom<String> for PackageName {
  type Error = PackageNameValidateError;
  fn try_from(value: String) -> Result<Self, Self::Error> {
    Self::new(value)
  }
}

impl std::fmt::Display for PackageName {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    write!(f, "{}", self.0)
  }
}

impl std::fmt::Debug for PackageName {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    write!(f, "{}", self.0)
  }
}

impl<'a> serde::Deserialize<'a> for PackageName {
  fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
  where
    D: serde::Deserializer<'a>,
  {
    let s: String = String::deserialize(deserializer)?;
    Self::new(s).map_err(serde::de::Error::custom)
  }
}

impl serde::Serialize for PackageName {
  fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
  where
    S: serde::Serializer,
  {
    self.0.serialize(serializer)
  }
}

impl sqlx::Decode<'_, Postgres> for PackageName {
  fn decode(
    value: PgValueRef<'_>,
  ) -> Result<Self, Box<dyn std::error::Error + 'static + Send + Sync>> {
    let s: String = sqlx::Decode::<'_, Postgres>::decode(value)?;
    Self::new(s).map_err(|e| Box::new(e) as _)
  }
}

impl<'q> sqlx::Encode<'q, Postgres> for PackageName {
  fn encode_by_ref(
    &self,
    buf: &mut <Postgres as sqlx::database::HasArguments<'q>>::ArgumentBuffer,
  ) -> sqlx::encode::IsNull {
    <std::string::String as sqlx::Encode<'_, Postgres>>::encode_by_ref(
      &self.0, buf,
    )
  }
}

impl sqlx::Type<Postgres> for PackageName {
  fn type_info() -> <Postgres as sqlx::Database>::TypeInfo {
    <String as sqlx::Type<Postgres>>::type_info()
  }
}

#[derive(Debug, Clone, Error)]
pub enum PackageNameValidateError {
  #[error("package name must be at least 2 characters long")]
  TooShort,

  #[error("package name must be at most 32 characters long")]
  TooLong,

  #[error("package name must contain only lowercase ascii alphanumeric characters and hyphens")]
  InvalidCharacters,

  #[error("package name must not start or end with a hyphen")]
  LeadingOrTrailingHyphens,

  #[error("package name must not contain double hyphens")]
  DoubleHyphens,
}

#[derive(Clone, PartialEq, Eq, Hash)]
pub struct ScopedPackageName {
  pub scope: ScopeName,
  pub package: PackageName,
}

#[derive(Debug, Clone, Error)]
pub enum ScopedPackageNameValidateError {
  #[error(transparent)]
  ScopeName(ScopeNameValidateError),

  #[error(transparent)]
  PackageName(PackageNameValidateError),

  #[error("scope must start with '@' sign")]
  MissingAtPrefix,

  #[error("scoped package name must contain '/' separator between scope and package name")]
  MissingSlashSeparator,
}

impl ScopedPackageName {
  pub fn new(
    scoped_name: String,
  ) -> Result<Self, ScopedPackageNameValidateError> {
    let Some(scoped_name) = scoped_name.strip_prefix('@') else {
      return Err(ScopedPackageNameValidateError::MissingAtPrefix);
    };
    let Some((scope, package)) = scoped_name.split_once('/') else {
      return Err(ScopedPackageNameValidateError::MissingSlashSeparator);
    };

    let scope = ScopeName::new(scope.to_string())
      .map_err(ScopedPackageNameValidateError::ScopeName)?;
    let package = PackageName::new(package.to_string())
      .map_err(ScopedPackageNameValidateError::PackageName)?;
    Ok(Self { scope, package })
  }
}

impl std::fmt::Display for ScopedPackageName {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    write!(f, "@{}/{}", self.scope, self.package)
  }
}

impl std::fmt::Debug for ScopedPackageName {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    write!(f, "@{}/{}", self.scope, self.package)
  }
}

impl<'a> serde::Deserialize<'a> for ScopedPackageName {
  fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
  where
    D: serde::Deserializer<'a>,
  {
    let s: String = String::deserialize(deserializer)?;
    Self::new(s).map_err(serde::de::Error::custom)
  }
}

impl serde::Serialize for ScopedPackageName {
  fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
  where
    S: serde::Serializer,
  {
    String::serialize(&format!("{}/{}", self.scope, self.package), serializer)
  }
}

/// A package version, like '1.2.3' or '0.0.0-foo'. The version is not prefixed
/// with a v.
/// The version must be a valid semver version.
#[derive(Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct Version(pub deno_semver::Version);

impl Version {
  pub fn new(specified: &str) -> Result<Self, VersionValidateError> {
    let version = deno_semver::Version::parse_standard(specified)
      .map_err(VersionValidateError::InvalidVersion)?;
    let normalized = version.to_string();
    if normalized != specified {
      return Err(VersionValidateError::NotNormalized {
        specified: specified.to_owned(),
        normalized,
      });
    }
    Ok(Version(version))
  }
}

impl TryFrom<&str> for Version {
  type Error = VersionValidateError;
  fn try_from(value: &str) -> Result<Self, Self::Error> {
    Self::new(value)
  }
}

impl TryFrom<String> for Version {
  type Error = VersionValidateError;
  fn try_from(value: String) -> Result<Self, Self::Error> {
    Self::new(&value)
  }
}

impl std::fmt::Display for Version {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    write!(f, "{}", self.0)
  }
}

impl std::fmt::Debug for Version {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    write!(f, "{:?}", self.0)
  }
}

impl<'a> serde::Deserialize<'a> for Version {
  fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
  where
    D: serde::Deserializer<'a>,
  {
    let s: String = String::deserialize(deserializer)?;
    Self::new(&s).map_err(serde::de::Error::custom)
  }
}

impl serde::Serialize for Version {
  fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
  where
    S: serde::Serializer,
  {
    self.0.to_string().serialize(serializer)
  }
}

impl sqlx::Decode<'_, Postgres> for Version {
  fn decode(
    value: PgValueRef<'_>,
  ) -> Result<Self, Box<dyn std::error::Error + 'static + Send + Sync>> {
    let s: String = sqlx::Decode::<'_, Postgres>::decode(value)?;
    Self::new(&s).map_err(|e| Box::new(e) as _)
  }
}

impl<'q> sqlx::Encode<'q, Postgres> for Version {
  fn encode_by_ref(
    &self,
    buf: &mut <Postgres as sqlx::database::HasArguments<'q>>::ArgumentBuffer,
  ) -> sqlx::encode::IsNull {
    let serialized = self.0.to_string();
    <std::string::String as sqlx::Encode<'_, Postgres>>::encode_by_ref(
      &serialized,
      buf,
    )
  }
}

impl sqlx::Type<Postgres> for Version {
  fn type_info() -> <Postgres as sqlx::Database>::TypeInfo {
    <String as sqlx::Type<Postgres>>::type_info()
  }
}

#[derive(Debug, Clone, Error)]
pub enum VersionValidateError {
  #[error("invalid semver version: {0}")]
  InvalidVersion(VersionParseError),

  #[error(
    "version must be normalized: expected {normalized}, got {specified}"
  )]
  NotNormalized {
    specified: String,
    normalized: String,
  },
}

/// A package path, like '/foo' or '/foo/bar'. The path is prefixed with a slash
/// and does not end with a slash.
///
/// The path must not contain any double slashes, dot segments, or dot dot
/// segments.
///
/// The path must be less than 155 characters long, including the slash prefix.
///
/// The path must not contain any windows reserved characters, like CON, PRN,
/// AUX, NUL, or COM1.
///
/// The path must not contain any windows path separators, like backslash or
/// colon.
///
/// The path must only contain ascii alphanumeric characters, and the characters
/// '$', '(', ')', '+', '-', '.', '@', '[', ']', '_', '{', '}',  '~'.
///
/// The path must not start with `/_dist/`, as this is the directory JSR will
/// emit `.d.ts` and `.js` files to when building the npm tarball.
///
/// Path's are case sensitive, and comparisons and hashing are also case
/// sensitive. However, to ensure no collisions based only on case-sensitivity,
/// one may use the `CaseInsensitivePackagePath` type to compare paths in a
/// case insensitive manner.
#[derive(Clone, Default)]
pub struct PackagePath(String);

impl PackagePath {
  pub fn new(path: String) -> Result<Self, PackagePathValidationError> {
    let len = path.len();
    // The total length of the path must be less than 160 characters to support
    // windows. We reduce this further to 155 to work around tarball
    // restrictions.
    if len > 155 {
      return Err(PackagePathValidationError::TooLong(len));
    }

    if len == 0 {
      return Err(PackagePathValidationError::MissingPrefix);
    }

    let mut components = path.split('/').peekable();

    let Some("") = components.next() else {
      return Err(PackagePathValidationError::MissingPrefix);
    };

    let mut last = None;
    let mut first = true;

    while let Some(component) = components.next() {
      last = Some(component);
      if component.is_empty() {
        if components.peek().is_none() {
          return Err(PackagePathValidationError::TrailingSlash);
        }
        return Err(PackagePathValidationError::EmptyComponent);
      }

      if component == "." || component == ".." {
        return Err(PackagePathValidationError::DotSegment);
      }

      if let Some(err) = component.chars().find_map(&mut valid_char) {
        return Err(err);
      }

      let basename = match component.rsplit_once('.') {
        Some((_, "")) => {
          return Err(PackagePathValidationError::TrailingDot(
            component.to_owned(),
          ));
        }
        Some((basename, _)) => basename,
        None => component,
      };

      let lower_basename = basename.to_ascii_lowercase();
      if WINDOWS_RESERVED_NAMES
        .binary_search(&&*lower_basename)
        .is_ok()
      {
        return Err(PackagePathValidationError::ReservedName(
          component.to_owned(),
        ));
      }

      if first && component.eq_ignore_ascii_case("_dist") {
        return Err(PackagePathValidationError::ReservedUnderscoreDist);
      }
      first = false;
    }

    // Due to restrictions in how tarballs are built, we need the ensure that
    // the last path component is less than 100 characters long. We further
    // reduce this to 95, to allow for modifying the extension (for example, we
    // add d.ts in places).
    let last = last.unwrap();
    if last.len() > 95 {
      return Err(PackagePathValidationError::LastPathComponentTooLong(
        last.len(),
      ));
    }

    Ok(Self(path))
  }

  pub fn case_insensitive(&self) -> CaseInsensitivePackagePath<'_> {
    CaseInsensitivePackagePath::new(Cow::Borrowed(self))
  }
}

impl PartialEq for PackagePath {
  fn eq(&self, other: &Self) -> bool {
    self.0 == other.0
  }
}

impl Eq for PackagePath {}

impl std::hash::Hash for PackagePath {
  fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
    self.0.hash(state);
  }
}

impl TryFrom<&str> for PackagePath {
  type Error = PackagePathValidationError;
  fn try_from(value: &str) -> Result<Self, Self::Error> {
    Self::new(value.to_owned())
  }
}

impl TryFrom<String> for PackagePath {
  type Error = PackagePathValidationError;
  fn try_from(value: String) -> Result<Self, Self::Error> {
    Self::new(value)
  }
}

impl std::ops::Deref for PackagePath {
  type Target = str;

  fn deref(&self) -> &Self::Target {
    &self.0
  }
}

impl std::fmt::Display for PackagePath {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    write!(f, "{}", self.0)
  }
}

impl std::fmt::Debug for PackagePath {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    write!(f, "{:?}", self.0)
  }
}

impl<'a> serde::Deserialize<'a> for PackagePath {
  fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
  where
    D: serde::Deserializer<'a>,
  {
    let s: String = String::deserialize(deserializer)?;
    Self::new(s).map_err(serde::de::Error::custom)
  }
}

impl serde::Serialize for PackagePath {
  fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
  where
    S: serde::Serializer,
  {
    self.0.serialize(serializer)
  }
}

impl sqlx::Decode<'_, Postgres> for PackagePath {
  fn decode(
    value: PgValueRef<'_>,
  ) -> Result<Self, Box<dyn std::error::Error + 'static + Send + Sync>> {
    let s: String = sqlx::Decode::<'_, Postgres>::decode(value)?;
    Self::new(s).map_err(|e| Box::new(e) as _)
  }
}

impl<'q> sqlx::Encode<'q, Postgres> for PackagePath {
  fn encode_by_ref(
    &self,
    buf: &mut <Postgres as sqlx::database::HasArguments<'q>>::ArgumentBuffer,
  ) -> sqlx::encode::IsNull {
    <std::string::String as sqlx::Encode<'_, Postgres>>::encode_by_ref(
      &self.0, buf,
    )
  }
}

impl sqlx::Type<Postgres> for PackagePath {
  fn type_info() -> <Postgres as sqlx::Database>::TypeInfo {
    <String as sqlx::Type<Postgres>>::type_info()
  }
}

const WINDOWS_RESERVED_NAMES: [&str; 22] = [
  "aux", "com1", "com2", "com3", "com4", "com5", "com6", "com7", "com8",
  "com9", "con", "lpt1", "lpt2", "lpt3", "lpt4", "lpt5", "lpt6", "lpt7",
  "lpt8", "lpt9", "nul", "prn",
];

fn valid_char(c: char) -> Option<PackagePathValidationError> {
  match c {
    'a'..='z'
    | 'A'..='Z'
    | '0'..='9'
    | '$'
    | '('
    | ')'
    | '+'
    | '-'
    | '.'
    | '@'
    | '['
    | ']'
    | '_'
    | '{'
    | '}'
    | '~' => None,
    // informative error messages for some invalid characters
    '\\' | ':' => Some(
      PackagePathValidationError::InvalidWindowsPathSeparatorChar(c),
    ),
    '<' | '>' | '"' | '|' | '?' | '*' => {
      Some(PackagePathValidationError::InvalidWindowsChar(c))
    }
    ' ' | '\t' | '\n' | '\r' => {
      Some(PackagePathValidationError::InvalidWhitespace(c))
    }
    '%' | '#' => Some(PackagePathValidationError::InvalidSpecialUrlChar(c)),
    // other invalid characters
    c => Some(PackagePathValidationError::InvalidOtherChar(c)),
  }
}

#[derive(Debug, Clone, Error, PartialEq)]
pub enum PackagePathValidationError {
  #[error("package path must be at most 155 characters long, but is {0} characters long")]
  TooLong(usize),

  #[error("the last path component must be at most 95 characters long, but is {0} characters long")]
  LastPathComponentTooLong(usize),

  #[error("package path must be prefixed with a slash")]
  MissingPrefix,

  #[error("package path must not end with a slash")]
  TrailingSlash,

  #[error("package path must not contain empty components")]
  EmptyComponent,

  #[error("package path must not contain dot segments like '.' or '..'")]
  DotSegment,

  #[error(
    "package path must not contain windows reserved names like 'CON' or 'PRN' (found '{0}')"
  )]
  ReservedName(String),

  #[error(
    "package path must not start with /_dist/, as this is the directory JSR will emit .d.ts and .js files to when building the npm tarball"
  )]
  ReservedUnderscoreDist,

  #[error("path segment must not end in a dot (found '{0}')")]
  TrailingDot(String),

  #[error(
    "package path must not contain windows path separators like '\\' or ':' (found '{0}')"
  )]
  InvalidWindowsPathSeparatorChar(char),

  #[error(
    "package path must not contain windows reserved characters like '<', '>', '\"', '|', '?', or '*' (found '{0}')"
  )]
  InvalidWindowsChar(char),

  #[error("package path must not contain whitespace (found '{}')", .0.escape_debug())]
  InvalidWhitespace(char),

  #[error("package path must not contain special URL characters (found '{}')", .0.escape_debug())]
  InvalidSpecialUrlChar(char),

  #[error("package path must not contain invalid characters (found '{}')", .0.escape_debug())]
  InvalidOtherChar(char),
}

/// Case insensitive package path. This type is useful for comparing package
/// paths in a case insensitive manner.
///
/// The hash and equality of this type are case insensitive.
#[derive(Clone, Default)]
pub struct CaseInsensitivePackagePath<'a> {
  path: Cow<'a, PackagePath>,
  lower: Option<String>,
}

impl<'a> CaseInsensitivePackagePath<'a> {
  pub fn new(path: Cow<'a, PackagePath>) -> Self {
    let has_uppercase = (*path).chars().any(|c| char::is_ascii_uppercase(&c));
    let lower = has_uppercase.then(|| path.to_ascii_lowercase());
    Self { path, lower }
  }

  pub fn into_inner(self) -> Cow<'a, PackagePath> {
    self.path
  }

  pub fn is_readme(&self) -> bool {
    let path =
      std::path::PathBuf::from(self.lower.as_deref().unwrap_or(&self.path));
    let name = path
      .file_stem()
      .and_then(|name| name.to_str())
      .unwrap_or_default();
    let extension = path
      .extension()
      .and_then(|ext| ext.to_str())
      .unwrap_or_default();
    let parent = path
      .parent()
      .and_then(|ext| ext.to_str())
      .unwrap_or_default();

    parent == "/"
      && name == "readme"
      && matches!(extension, "md" | "txt" | "markdown")
  }

  pub fn to_owned(&self) -> CaseInsensitivePackagePath<'static> {
    CaseInsensitivePackagePath {
      path: Cow::Owned(PackagePath::clone(&self.path)),
      lower: self.lower.clone(),
    }
  }
}

impl PartialEq for CaseInsensitivePackagePath<'_> {
  fn eq(&self, other: &Self) -> bool {
    let self_lower = self.lower.as_deref().unwrap_or(&self.path);
    let other_lower = other.lower.as_deref().unwrap_or(&other.path);
    self_lower == other_lower
  }
}

impl Eq for CaseInsensitivePackagePath<'_> {}

impl std::hash::Hash for CaseInsensitivePackagePath<'_> {
  fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
    let self_lower = self.lower.as_deref().unwrap_or(&self.path);
    self_lower.hash(state);
  }
}

impl std::fmt::Display for CaseInsensitivePackagePath<'_> {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    std::fmt::Display::fmt(&self.path, f)
  }
}

impl std::fmt::Debug for CaseInsensitivePackagePath<'_> {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    std::fmt::Debug::fmt(&self.path, f)
  }
}

#[cfg(test)]
mod tests {
  use std::collections::HashSet;

  use super::*;

  #[test]
  fn test_scope_name() {
    // Test valid scope names
    assert!(ScopeName::try_from("ry").is_ok());
    assert!(ScopeName::try_from("foo").is_ok());
    assert!(ScopeName::try_from("foo-bar").is_ok());
    assert!(ScopeName::try_from("foo-bar-baz").is_ok());
    assert!(ScopeName::try_from("foo-123").is_ok());
    assert!(ScopeName::try_from("foo-123-bar").is_ok());
    assert!(ScopeName::try_from("f123").is_ok());
    assert!(ScopeName::try_from("foo-bar-baz-qux").is_ok());

    // Test invalid scope names
    assert!(ScopeName::try_from("").is_err());
    assert!(ScopeName::try_from("f").is_err());
    assert!(ScopeName::try_from("Foo").is_err());
    assert!(ScopeName::try_from("oooF").is_err());
    assert!(ScopeName::try_from("very-long-name-is-very-long").is_err());
    assert!(ScopeName::try_from("foo_bar").is_err());
    assert!(ScopeName::try_from("-foo").is_err());
    assert!(ScopeName::try_from("foo-").is_err());
    assert!(ScopeName::try_from("foo--bar").is_err());
    assert!(ScopeName::try_from("foo-123-").is_err());
    assert!(ScopeName::try_from("-123-foo").is_err());
    assert!(ScopeName::try_from("foo-123-bar-").is_err());
    assert!(ScopeName::try_from("@foo").is_err());
  }

  #[test]
  fn test_package_name() {
    // Test valid package names
    assert!(PackageName::try_from("fo").is_ok());
    assert!(PackageName::try_from("foo").is_ok());
    assert!(PackageName::try_from("foo-bar").is_ok());
    assert!(PackageName::try_from("foo-bar-baz").is_ok());
    assert!(PackageName::try_from("foo-123").is_ok());
    assert!(PackageName::try_from("foo-123-bar").is_ok());
    assert!(PackageName::try_from("f123").is_ok());
    assert!(PackageName::try_from("foo-bar-baz-qux").is_ok());
    assert!(PackageName::try_from("very-long-name-is-very-long").is_ok());

    // Test invalid package names
    assert!(PackageName::try_from("").is_err());
    assert!(PackageName::try_from("f").is_err());
    assert!(PackageName::try_from("Foo").is_err());
    assert!(PackageName::try_from("oooF").is_err());
    assert!(
      PackageName::try_from("very-long-name-is-very-very-very-long").is_err()
    );
    assert!(PackageName::try_from("foo_bar").is_err());
    assert!(PackageName::try_from("-foo").is_err());
    assert!(PackageName::try_from("foo-").is_err());
    assert!(PackageName::try_from("foo--bar").is_err());
    assert!(PackageName::try_from("foo-123-").is_err());
    assert!(PackageName::try_from("-123-foo").is_err());
    assert!(PackageName::try_from("foo-123-bar-").is_err());
    assert!(PackageName::try_from("foo@").is_err());
  }

  #[test]
  fn test_version() {
    // Test valid versions
    assert!(Version::new("1.2.3").is_ok());
    assert!(Version::new("1.2.3-alpha.0").is_ok());
    assert!(Version::new("1.2.3-alpha.0").is_ok());

    // Test invalid versions
    assert!(Version::new("1.2").is_err());
    assert!(Version::new("").is_err());
    assert!(Version::new("0..0").is_err());

    // Test not normalized versions
    assert!(Version::new("v1.2.3").is_err());
    assert!(Version::new(" v1.2.3").is_err());
    assert!(Version::new("v1.2.3 ").is_err());
    assert!(Version::new("v1.2.3 ").is_err());
  }

  #[test]
  fn test_scoped_package_name() {
    // Test valid scoped package names
    assert!(ScopedPackageName::new("@scope/foo".to_string()).is_ok());
    assert!(ScopedPackageName::new("@scope/foo-bar".to_string()).is_ok());
    assert!(ScopedPackageName::new("@scope-scope/foo".to_string()).is_ok());
    assert!(ScopedPackageName::new("@scope-scope/foo-123".to_string()).is_ok());

    // Test invalid scoped package names
    assert!(ScopedPackageName::new("scope/foo".to_string()).is_err());
    assert!(ScopedPackageName::new("@scope_scope/foo".to_string()).is_err());
    assert!(ScopedPackageName::new("@scope".to_string()).is_err());
    assert!(ScopedPackageName::new("@scope/bar_bar".to_string()).is_err());
    assert!(ScopedPackageName::new("@scope/".to_string()).is_err());
    assert!(ScopedPackageName::new("@scope/foo/bar".to_string()).is_err());
  }

  #[test]
  fn test_package_path_lengths() {
    fn mock_package_path(
      path_segments: &[usize],
    ) -> Result<PackagePath, PackagePathValidationError> {
      let mut path = String::new();
      for s in path_segments.iter() {
        let path_segment = "a".repeat(*s);
        path.push('/');
        path.push_str(&path_segment);
      }
      PackagePath::new(path)
    }

    mock_package_path(&[58, 95]).unwrap();
    assert_eq!(
      mock_package_path(&[59, 95]).unwrap_err(),
      PackagePathValidationError::TooLong(156)
    );
    assert_eq!(
      mock_package_path(&[57, 96]).unwrap_err(),
      PackagePathValidationError::LastPathComponentTooLong(96)
    );
    mock_package_path(&[56, 95, 1]).unwrap();
    mock_package_path(&[30, 94]).unwrap();
    assert_eq!(
      mock_package_path(&[1, 96]).unwrap_err(),
      PackagePathValidationError::LastPathComponentTooLong(96)
    );
    mock_package_path(&[96, 57]).unwrap();
  }

  #[test]
  fn test_package_path() {
    // Test valid package paths
    assert!(PackagePath::try_from("/foo").is_ok());
    assert!(PackagePath::try_from("/Foo").is_ok());
    assert!(PackagePath::try_from("/FOOO").is_ok());
    assert!(PackagePath::try_from("/foo/bar").is_ok());
    assert!(PackagePath::try_from("/foo/bar/baz.ts").is_ok());
    assert!(PackagePath::try_from("/foo/[scope]/baz.ts").is_ok());
    assert!(PackagePath::try_from("/foo.test.ts").is_ok());
    assert!(PackagePath::try_from("/foo+bar").is_ok());
    assert!(PackagePath::try_from("/foo-bar").is_ok());
    assert!(PackagePath::try_from("/foo-bar-baz").is_ok());
    assert!(PackagePath::try_from("/foo-123").is_ok());
    assert!(PackagePath::try_from("/123").is_ok());
    assert!(PackagePath::try_from("/{section}").is_ok());
    assert!(PackagePath::try_from("/my$foo").is_ok());
    assert!(PackagePath::try_from("/hello@version").is_ok());
    assert!(PackagePath::try_from("/foo_bar").is_ok());
    assert!(PackagePath::try_from("/~/foo").is_ok());
    assert!(PackagePath::try_from("/~foo/bar").is_ok());
    assert!(PackagePath::try_from("/(section)/32").is_ok());
    assert!(PackagePath::try_from("/foo/_dist/23").is_ok());

    // Test invalid package paths
    assert!(PackagePath::try_from("").is_err());
    assert!(PackagePath::try_from("foo").is_err());
    assert!(PackagePath::try_from("/").is_err());
    assert!(PackagePath::try_from("/foo/").is_err());
    assert!(PackagePath::try_from("/foo//bar").is_err());
    assert!(PackagePath::try_from("//bar").is_err());
    assert!(PackagePath::try_from("/foo/bar/").is_err());
    assert!(PackagePath::try_from("/./foo").is_err());
    assert!(PackagePath::try_from("/../foo").is_err());
    assert!(PackagePath::try_from("/foo/./bar").is_err());
    assert!(PackagePath::try_from("/foo/../bar").is_err());
    assert!(PackagePath::try_from("/foo/bar/.").is_err());
    assert!(PackagePath::try_from("/foo/bar/..").is_err());
    assert!(PackagePath::try_from("/bar!").is_err());
    assert!(PackagePath::try_from("/?asd").is_err());
    assert!(PackagePath::try_from("/foo?asd").is_err());
    assert!(PackagePath::try_from("/foo#asd").is_err());
    assert!(PackagePath::try_from("/#").is_err());
    assert!(PackagePath::try_from("/foo&bar").is_err());
    assert!(PackagePath::try_from("/^").is_err());
    assert!(PackagePath::try_from("/<foo>").is_err());
    assert!(PackagePath::try_from("/using\\backslashes").is_err());
    assert!(PackagePath::try_from("/using spaces").is_err());
    assert!(PackagePath::try_from("/using\ttabs").is_err());
    assert!(PackagePath::try_from("/using\nnewlines").is_err());
    assert!(PackagePath::try_from("/using\rcarriagereturn").is_err());
    assert!(PackagePath::try_from("/con").is_err());
    assert!(PackagePath::try_from("/CON").is_err());
    assert!(PackagePath::try_from("/com1").is_err());
    assert!(PackagePath::try_from("/aux").is_err());
    assert!(PackagePath::try_from("/con.txt").is_err());
    assert!(PackagePath::try_from("/CON.txt").is_err());
    assert!(PackagePath::try_from("/foo.").is_err());
    assert!(PackagePath::try_from("/f".repeat(81)).is_err());
    assert!(PackagePath::try_from("/_dist").is_err());
    assert!(PackagePath::try_from("/_dist/foo.txt").is_err());
  }

  #[test]
  fn test_package_path_compare_and_hash() {
    let a = PackagePath::try_from("/foo").unwrap();
    let a_capitalized = PackagePath::try_from("/Foo").unwrap();
    let b = PackagePath::try_from("/bar").unwrap();

    assert_eq!(a, a);
    assert_ne!(a, a_capitalized);
    assert_ne!(a, b);
    assert_ne!(a_capitalized, b);

    let mut set = HashSet::new();
    assert!(set.insert(a.clone()));
    assert!(!set.insert(a.clone()));
    assert!(set.insert(a_capitalized.clone()));
    assert!(set.insert(b.clone()));

    assert!(set.contains(&a));
    assert!(set.contains(&a_capitalized));
    assert!(set.contains(&b));

    let a_case_insensitive = a.case_insensitive();
    let a_capitalized_case_insensitive = a_capitalized.case_insensitive();
    let b_case_insensitive = b.case_insensitive();

    assert_eq!(a_case_insensitive, a_case_insensitive);
    assert_eq!(a_case_insensitive, a_capitalized_case_insensitive);
    assert_ne!(a_case_insensitive, b_case_insensitive);

    let mut set = HashSet::new();
    assert!(set.insert(a_case_insensitive.clone()));
    assert!(!set.insert(a_case_insensitive.clone()));
    assert!(!set.insert(a_capitalized_case_insensitive.clone()));
    assert!(set.insert(b_case_insensitive.clone()));

    assert!(set.contains(&a_case_insensitive));
    assert!(set.contains(&a_capitalized_case_insensitive));
    assert!(set.contains(&b_case_insensitive));
  }

  #[test]
  fn test_package_path_is_readme() {
    // Valid READMEs
    assert!(PackagePath::try_from("/README.md")
      .unwrap()
      .case_insensitive()
      .is_readme());
    assert!(PackagePath::try_from("/README.txt")
      .unwrap()
      .case_insensitive()
      .is_readme());
    assert!(PackagePath::try_from("/README.markdown")
      .unwrap()
      .case_insensitive()
      .is_readme());
    assert!(PackagePath::try_from("/readme.md")
      .unwrap()
      .case_insensitive()
      .is_readme());
    assert!(PackagePath::try_from("/readme.txt")
      .unwrap()
      .case_insensitive()
      .is_readme());
    assert!(PackagePath::try_from("/readme.markdown")
      .unwrap()
      .case_insensitive()
      .is_readme());
    assert!(PackagePath::try_from("/ReAdMe.md")
      .unwrap()
      .case_insensitive()
      .is_readme());

    // Invalid READMEs
    assert!(!PackagePath::try_from("/foo/README.md")
      .unwrap()
      .case_insensitive()
      .is_readme());
    assert!(!PackagePath::try_from("/foo.md")
      .unwrap()
      .case_insensitive()
      .is_readme());
  }
}
