ALTER TABLE package_versions ADD COLUMN user_id uuid REFERENCES users (id);
