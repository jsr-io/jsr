ALTER TABLE scope_members ADD COLUMN is_admin BOOLEAN NOT NULL DEFAULT FALSE;
UPDATE scope_members SET is_admin = true WHERE is_owner = true;
ALTER TABLE scope_members ADD CONSTRAINT valid_admin_owner CHECK ((is_owner AND is_admin) OR is_owner = false);
