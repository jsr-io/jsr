CREATE TABLE oauth_states (
  csrf_token text NOT NULL PRIMARY KEY,
  pkce_code_verifier text NOT NULL,
  redirect_url text NOT NULL,
  updated_at timestamptz NOT NULL DEFAULT now(),
  created_at timestamptz NOT NULL DEFAULT now()
);
SELECT manage_updated_at('oauth_states');
