CREATE TYPE package_readme_source AS ENUM ('readme', 'jsdoc');

ALTER TABLE packages ADD COLUMN readme_source package_readme_source NOT NULL DEFAULT 'jsdoc';
