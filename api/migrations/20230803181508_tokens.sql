CREATE TYPE token_type AS ENUM ('web');

CREATE TABLE tokens (
  id UUID NOT NULL PRIMARY KEY DEFAULT uuid_generate_v4(),
  hash TEXT NOT NULL UNIQUE,
  user_id UUID NOT NULL REFERENCES users (id) ON DELETE CASCADE,
  type token_type NOT NULL,
  description TEXT,
  expires_at TIMESTAMPTZ,
  updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
  created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);
SELECT manage_updated_at('tokens');
