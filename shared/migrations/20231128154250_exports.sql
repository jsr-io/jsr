-- Add `exports` column to `package_versions` table that is jsonb non-nullable,
-- and populate it with the value of `main_module` column as `{ ".": ".${main_module}" }`.
-- Then, drop the `main_module` column.
ALTER TABLE package_versions
ADD COLUMN exports jsonb NOT NULL DEFAULT '{}'::jsonb;
UPDATE package_versions
SET exports = jsonb_build_object('.', concat('.', main_module));
ALTER TABLE package_versions DROP COLUMN main_module;