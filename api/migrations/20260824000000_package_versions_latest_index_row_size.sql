-- Drop INCLUDE (meta) from the latest-version index. INCLUDE columns are
-- stored inline in the b-tree index tuple and can never be TOASTed
-- out-of-line, so an index row is hard-capped at ~8191 bytes (after
-- compression). A `meta` above that cap (e.g. `entrypoints_without_docs`
-- listing thousands of undocumented entrypoints) made the INSERT INTO
-- package_versions fail, permanently wedging the publishing task in
-- `pending` (jsr-io/jsr#1505). The lateral latest-meta lookup this covered
-- is a LIMIT 1 probe, so losing the index-only scan costs a single heap
-- fetch per package.
DROP INDEX IF EXISTS idx_package_versions_latest;
CREATE INDEX idx_package_versions_latest ON package_versions (scope, name, version DESC)
  WHERE is_yanked = false AND version NOT LIKE '%-%';
