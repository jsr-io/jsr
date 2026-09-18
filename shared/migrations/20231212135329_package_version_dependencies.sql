CREATE TYPE dependency_kind AS ENUM ('jsr', 'npm');
CREATE TABLE package_version_dependencies (
  package_scope TEXT NOT NULL REFERENCES scopes (scope),
  package_name TEXT NOT NULL,
  package_version TEXT NOT NULL,
  dependency_kind dependency_kind NOT NULL,
  dependency_name TEXT NOT NULL,
  dependency_constraint TEXT NOT NULL,
  dependency_path TEXT NOT NULL,
  updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
  created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
  PRIMARY KEY (
    package_scope,
    package_name,
    package_version,
    dependency_kind,
    dependency_name,
    dependency_constraint,
    dependency_path
  ),
  FOREIGN KEY (package_scope, package_name) REFERENCES packages (scope, name),
  FOREIGN KEY (package_scope, package_name, package_version) REFERENCES package_versions (scope, name, version)
);
CREATE INDEX package_version_dependencies_package_scope_package_name_package_version_idx ON package_version_dependencies (package_scope, package_name, package_version);
CREATE INDEX package_version_dependencies_dependency_kind_dependency_name_idx ON package_version_dependencies (dependency_kind, dependency_name);
SELECT manage_updated_at('package_version_dependencies');