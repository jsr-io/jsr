CREATE TYPE change_type AS ENUM (
    'PACKAGE_VERSION_ADDED',
    'PACKAGE_TAG_ADDED'
);

CREATE TABLE changes (
    seq BIGSERIAL PRIMARY KEY,
    change_type change_type NOT NULL,
    scope_name text NOT NULL,
    package_name text NOT NULL,
    data TEXT NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX changes_scope_name_idx ON changes (scope_name, package_name);
CREATE INDEX changes_created_at_idx ON changes (created_at);
