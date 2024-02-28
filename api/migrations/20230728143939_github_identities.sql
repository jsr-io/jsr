CREATE TABLE github_identities (
    github_id bigint NOT NULL UNIQUE,
    access_token text,
    access_token_expires_at timestamp with time zone,
    refresh_token text,
    refresh_token_expires_at timestamp with time zone,
    updated_at timestamp with time zone DEFAULT now() NOT NULL,
    created_at timestamp with time zone DEFAULT now() NOT NULL
);
SELECT manage_updated_at('github_identities');
ALTER TABLE users ADD COLUMN github_id bigint;
ALTER TABLE users ADD CONSTRAINT users_github_identities_fk FOREIGN KEY (github_id) REFERENCES github_identities(github_id);

