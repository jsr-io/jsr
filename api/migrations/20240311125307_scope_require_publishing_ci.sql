ALTER TABLE scopes
  ADD COLUMN require_publishing_from_ci BOOLEAN NOT NULL DEFAULT FALSE;
