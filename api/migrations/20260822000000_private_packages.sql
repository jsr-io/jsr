ALTER TABLE packages ADD COLUMN is_private BOOLEAN NOT NULL DEFAULT FALSE;
CREATE INDEX idx_packages_is_private ON packages(scope, is_private);
