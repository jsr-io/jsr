-- Add rekor_log_id to package_versions table
ALTER TABLE package_versions ADD COLUMN rekor_log_id TEXT;
