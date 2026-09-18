CREATE TABLE gitlab_identities (
   gitlab_id bigint NOT NULL UNIQUE,
   access_token text,
   access_token_expires_at timestamp with time zone,
   refresh_token text,
   updated_at timestamp with time zone DEFAULT now() NOT NULL,
   created_at timestamp with time zone DEFAULT now() NOT NULL
);
SELECT manage_updated_at('gitlab_identities');
ALTER TABLE users ADD COLUMN gitlab_id bigint;
ALTER TABLE users ADD CONSTRAINT users_gitlab_identities_fk FOREIGN KEY (gitlab_id) REFERENCES gitlab_identities(gitlab_id);
ALTER TABLE users ADD CONSTRAINT gitlab_id_unique UNIQUE (gitlab_id);
