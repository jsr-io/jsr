CREATE TABLE authorizations (
  exchange_token text NOT NULL PRIMARY KEY,
  code text NOT NULL,

  challenge text NOT NULL,
  permissions jsonb,
  approved boolean DEFAULT NULL,
  user_id uuid REFERENCES users(id) ON DELETE CASCADE DEFAULT NULL,

  expires_at timestamp with time zone NOT NULL,
  
  created_at timestamp with time zone NOT NULL DEFAULT now(),
  updated_at timestamp with time zone NOT NULL DEFAULT now(),

  CONSTRAINT user_id_approved_null CHECK ((user_id IS NULL AND approved IS NULL) OR (user_id IS NOT NULL AND approved IS NOT NULL))
);
SELECT manage_updated_at('authorizations');

CREATE UNIQUE INDEX authorizations_code_idx ON authorizations (code);
