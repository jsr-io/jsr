ALTER TABLE users ALTER COLUMN scope_limit DROP NOT NULL;

INSERT INTO users (id, name, avatar_url, is_staff, scope_limit)
VALUES (
       '00000000-0000-0000-0000-000000000000',
       'JSR',
       '/logo-square.svg',
       true,
       null
    )
ON CONFLICT (id) DO UPDATE
    SET name = EXCLUDED.name,
        avatar_url = EXCLUDED.avatar_url,
        is_staff = EXCLUDED.is_staff,
        scope_limit = EXCLUDED.scope_limit;
