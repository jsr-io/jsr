UPDATE packages SET runtime_compat = '{}'::jsonb WHERE runtime_compat IS NULL;
ALTER TABLE packages ALTER COLUMN runtime_compat SET NOT NULL;
