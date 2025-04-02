CREATE TYPE change_type AS ENUM (
    'PACKAGE_VERSION_ADDED',
    'PACKAGE_TAG_ADDED'
);

CREATE TABLE changes (
    seq BIGSERIAL PRIMARY KEY,
    change_type change_type NOT NULL,
    package_id VARCHAR(255) NOT NULL,
    data TEXT NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX changes_package_id_idx ON changes (package_id);
CREATE INDEX changes_created_at_idx ON changes (created_at);
