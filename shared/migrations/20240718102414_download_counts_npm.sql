CREATE TYPE download_kind AS ENUM ('npm_tgz', 'jsr_meta'); 

ALTER TABLE version_download_counts_4h
  DROP CONSTRAINT version_download_counts_4h_pkey,
  ADD COLUMN download_kind download_kind NOT NULL DEFAULT 'jsr_meta',
  ADD PRIMARY KEY (scope, package, version, time_bucket, download_kind);

ALTER TABLE version_download_counts_24h
  DROP CONSTRAINT version_download_counts_24h_pkey,
  ADD COLUMN download_kind download_kind NOT NULL DEFAULT 'jsr_meta',
  ADD PRIMARY KEY (scope, package, version, time_bucket, download_kind);