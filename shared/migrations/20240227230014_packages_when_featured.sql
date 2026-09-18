-- Add when_features to packages table
ALTER TABLE packages ADD COLUMN when_featured timestamptz;
