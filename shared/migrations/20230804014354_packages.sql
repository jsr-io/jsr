ALTER TABLE publishing_tasks
    ALTER COLUMN scope SET NOT NULL,
    ALTER COLUMN package SET NOT NULL,
    ALTER COLUMN user_id SET NOT NULL;

-- Here are the relations:
-- scopes < packages < package_versions < package_files

CREATE TABLE scopes (
    scope text NOT NULL PRIMARY KEY,
    creator uuid REFERENCES users (id) ON DELETE CASCADE,
    updated_at timestamptz NOT NULL DEFAULT now(),
    created_at timestamptz NOT NULL DEFAULT now()
);
SELECT manage_updated_at('scopes');

CREATE TABLE scope_members (
    scope text NOT NULL REFERENCES scopes (scope),
    user_id uuid NOT NULL REFERENCES users (id),
    is_owner boolean NOT NULL DEFAULT false,
    updated_at timestamptz NOT NULL DEFAULT now(),
    created_at timestamptz NOT NULL DEFAULT now(),
    UNIQUE (scope, user_id)
);
SELECT manage_updated_at('scope_members');
CREATE INDEX idx_scope_members_scope_user_id ON scope_members (scope, user_id);

CREATE TABLE packages (
    scope text NOT NULL REFERENCES scopes (scope),
    name text NOT NULL,
    updated_at timestamptz NOT NULL DEFAULT now(),
    created_at timestamptz NOT NULL DEFAULT now(),
    PRIMARY KEY (scope, name)
);
SELECT manage_updated_at('packages');

CREATE COLLATION en_natural (
  LOCALE = 'en-US-u-kn-true',
  PROVIDER = 'icu'
);

CREATE TABLE package_versions (
    scope text NOT NULL,
    name text NOT NULL,
    version text NOT NULL collate en_natural,
    main_module text NOT NULL,
    is_yanked boolean NOT NULL DEFAULT false,
    updated_at timestamptz NOT NULL DEFAULT now(),
    created_at timestamptz NOT NULL DEFAULT now(),
    PRIMARY KEY (scope, name, version),
    FOREIGN KEY (scope, name) REFERENCES packages (scope, name)
);
SELECT manage_updated_at('package_versions');

CREATE TABLE package_files (
    scope text NOT NULL,
    name text NOT NULL,
    version text NOT NULL,
    path text NOT NULL,
    size integer CHECK (size >= 0) NOT NULL,
    checksum text,
    updated_at timestamptz NOT NULL DEFAULT now(),
    created_at timestamptz NOT NULL DEFAULT now(),
    PRIMARY KEY (scope, name, version, path),
    FOREIGN KEY (scope, name, version) REFERENCES package_versions (scope, name, version) ON DELETE CASCADE
);
SELECT manage_updated_at('package_files');
CREATE INDEX idx_package_files_package_version ON package_files (scope, name, version, path);

-- test data

INSERT INTO scopes (scope, creator, updated_at, created_at)
VALUES ('test-scope', '00000000-0000-0000-0000-000000000000', now(), now());

INSERT INTO scope_members (scope, user_id, is_owner, updated_at, created_at)
VALUES ('test-scope', '00000000-0000-0000-0000-000000000000', true, now(), now());
