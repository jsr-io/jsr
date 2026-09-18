ALTER TABLE github_repositories ALTER COLUMN created_at SET DEFAULT now();
ALTER TABLE github_repositories ALTER COLUMN updated_at SET DEFAULT now();