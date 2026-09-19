-- The "latest version" of a package is now the latest unyanked stable
-- version, falling back to the latest unyanked prerelease version for
-- packages that have no stable release. Replace the stable-only partial
-- index with one that matches the new
-- `ORDER BY (version NOT LIKE '%-%') DESC, version DESC` pattern so the
-- lateral join for the latest version stays an index scan. `meta` is
-- deliberately not INCLUDEd: index rows are hard-capped at ~8191 bytes,
-- which a large `meta` exceeds (see
-- 20260824000000_package_versions_latest_index_row_size.sql).
DROP INDEX IF EXISTS idx_package_versions_latest;
CREATE INDEX idx_package_versions_latest ON package_versions (scope, name, (version NOT LIKE '%-%') DESC, version DESC)
  WHERE is_yanked = false;
