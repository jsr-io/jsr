CREATE EXTENSION IF NOT EXISTS "uuid-ossp";

-- Sets up a trigger for the given table to automatically set a column called
-- `updated_at` whenever the row is modified (unless `updated_at` was included
-- in the modified columns)
--
-- # Example
--
-- ```sql
-- CREATE TABLE users (id SERIAL PRIMARY KEY, updated_at TIMESTAMP NOT NULL DEFAULT NOW());
--
-- SELECT manage_updated_at('users');
-- ```
CREATE OR REPLACE FUNCTION manage_updated_at(_tbl regclass) RETURNS VOID AS $$
BEGIN
    EXECUTE format('CREATE TRIGGER set_updated_at BEFORE UPDATE ON %s
                    FOR EACH ROW EXECUTE PROCEDURE set_updated_at()', _tbl);
END;
$$ LANGUAGE plpgsql;

CREATE OR REPLACE FUNCTION set_updated_at() RETURNS trigger AS $$
BEGIN
    IF (
        NEW IS DISTINCT FROM OLD AND
        NEW.updated_at IS NOT DISTINCT FROM OLD.updated_at
    ) THEN
        NEW.updated_at := current_timestamp;
    END IF;
    RETURN NEW;
END;
$$ LANGUAGE plpgsql;

CREATE TABLE users (
    id uuid NOT NULL PRIMARY KEY DEFAULT uuid_generate_v4(),
    login text NOT NULL,
    name text NOT NULL,
    avatar_url text NOT NULL,
    updated_at timestamptz NOT NULL DEFAULT now(),
    created_at timestamptz NOT NULL DEFAULT now()
);
SELECT manage_updated_at('users');
CREATE INDEX idx_users_login ON users (login);
INSERT INTO users (id, login, name, avatar_url)
VALUES (
    '00000000-0000-0000-0000-000000000000',
    '!mockuser',
    'Mock User',
    'https://example.com/mockuser/avatar.jpg'
);

CREATE TYPE task_status AS ENUM ('pending', 'processing', 'completed', 'failed');
CREATE TABLE publishing_tasks (
    id uuid NOT NULL PRIMARY KEY DEFAULT uuid_generate_v4(),
    status task_status NOT NULL DEFAULT 'pending',
    user_id uuid REFERENCES users (id) ON DELETE SET NULL,
    scope text,
    package text,
    updated_at timestamptz NOT NULL DEFAULT now(),
    created_at timestamptz NOT NULL DEFAULT now()
);
SELECT manage_updated_at('publishing_tasks');
CREATE INDEX idx_publishing_tasks_created_at ON publishing_tasks (created_at);