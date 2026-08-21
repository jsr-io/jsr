-- Replace the latest-version partial index with a covering index that INCLUDEs meta.
-- This allows the lateral join for latest version + meta to be an index-only scan,
-- eliminating heap lookups for the most common package query pattern.
DROP INDEX IF EXISTS idx_package_versions_latest;
CREATE INDEX idx_package_versions_latest ON package_versions (scope, name, version DESC)
  INCLUDE (meta)
  WHERE is_yanked = false AND version NOT LIKE '%-%';

-- Composite index for list_packages_by_scope ordering pattern:
-- WHERE scope = $1 AND is_archived = ... ORDER BY is_archived ASC, name
CREATE INDEX idx_packages_scope_archived_name ON packages (scope, is_archived ASC, name ASC);

-- Replace the dependency lookup index: use full key columns (not INCLUDE) so PostgreSQL
-- can produce sorted output for the DISTINCT operation instead of hash-based dedup.
DROP INDEX IF EXISTS package_version_dependencies_dependency_kind_dependency_name_idx;
CREATE INDEX idx_package_version_deps_kind_name ON package_version_dependencies (dependency_kind, dependency_name, package_scope, package_name);

-- Covering index for get_version_downloads_24h (per-version query).
-- The PK already covers this but a narrower index with time_bucket ordering helps range scans.
DROP INDEX IF EXISTS version_download_counts_24h_package_idx;
CREATE INDEX idx_download_counts_24h_package ON version_download_counts_24h (scope, package, time_bucket)
  INCLUDE (version, kind, count);

-- Package-level download counts rollup table. Eliminates the need for
-- get_package_downloads_24h to SUM across all versions at read time.
-- For a package with 100 versions over 90 days, this reduces the scan
-- from ~18,000 rows to ~180 rows (no GROUP BY or SUM needed).
CREATE TABLE package_download_counts_24h (
  scope TEXT NOT NULL REFERENCES scopes (scope) ON DELETE CASCADE,
  package TEXT NOT NULL,
  time_bucket TIMESTAMP WITH TIME ZONE NOT NULL,
  kind download_kind NOT NULL,
  count INTEGER NOT NULL,
  PRIMARY KEY (scope, package, time_bucket, kind),
  FOREIGN KEY (scope, package) REFERENCES packages (scope, name) ON DELETE CASCADE
);

-- Covering index for count_package_dependencies (350k calls/hr).
-- INCLUDEs dependency_name so COUNT(DISTINCT dependency_name) is an index-only scan.
DROP INDEX IF EXISTS package_version_dependencies_package_scope_package_name_package_version_idx;
CREATE INDEX idx_pvd_scope_name_version ON package_version_dependencies (package_scope, package_name, package_version)
  INCLUDE (dependency_name);

-- Backfill from existing version-level data
INSERT INTO package_download_counts_24h (scope, package, time_bucket, kind, count)
SELECT scope, package, time_bucket, kind, SUM(count)
FROM version_download_counts_24h
GROUP BY scope, package, time_bucket, kind;
