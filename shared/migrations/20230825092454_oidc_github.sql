CREATE TABLE github_repositories (
  id bigint NOT NULL PRIMARY KEY,
  owner text NOT NULL,
  name text NOT NULL,
  created_at timestamptz NOT NULL,
  updated_at timestamptz NOT NULL
);
SELECT manage_updated_at('github_repositories');

ALTER TABLE packages ADD COLUMN github_repository_id bigint REFERENCES github_repositories(id) ON DELETE SET NULL;
