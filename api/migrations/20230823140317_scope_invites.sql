CREATE TABLE scope_invites (
  target_user_id uuid NOT NULL REFERENCES users (id),
  requesting_user_id uuid NOT NULL REFERENCES users (id),
  scope text NOT NULL REFERENCES scopes (scope),
  updated_at timestamptz NOT NULL DEFAULT now(),
  created_at timestamptz NOT NULL DEFAULT now()
);
SELECT manage_updated_at('scope_invites');
