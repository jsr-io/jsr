-- Replace the existing index on (dependency_kind, dependency_name) with a
-- covering index that includes (package_scope, package_name).  This turns the
-- expensive dependent-count query into an index-only scan.
DROP INDEX IF EXISTS package_version_dependencies_dependency_kind_dependency_name_idx;
CREATE INDEX package_version_dependencies_dep_kind_name_scope_pkg_idx
  ON package_version_dependencies (dependency_kind, dependency_name, package_scope, package_name);
