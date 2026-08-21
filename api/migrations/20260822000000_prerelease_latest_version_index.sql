-- The "latest version" of a package is now the latest unyanked stable
-- version, falling back to the latest unyanked prerelease version for
-- packages that have no stable release. Replace the stable-only partial
-- index with one that matches the new
-- `ORDER BY (version NOT LIKE '%-%') DESC, version DESC` pattern so the
-- lateral join for latest version + meta stays an index-only scan.
DROP INDEX IF EXISTS idx_package_versions_latest;
CREATE INDEX idx_package_versions_latest ON package_versions (scope, name, (version NOT LIKE '%-%') DESC, version DESC)
  INCLUDE (meta)
  WHERE is_yanked = false;
