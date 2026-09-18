ALTER TABLE version_download_counts_4h
  RENAME COLUMN download_kind TO kind;

ALTER TABLE version_download_counts_24h
  RENAME COLUMN download_kind TO kind;
