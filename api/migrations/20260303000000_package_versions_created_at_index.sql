-- Index for "recently updated" version listings (package_stats updated query)
-- Used in: ORDER BY package_versions.created_at DESC LIMIT 10
CREATE INDEX idx_package_versions_created_at ON package_versions (created_at DESC);
