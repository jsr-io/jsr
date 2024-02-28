ALTER TABLE users ADD COLUMN email VARCHAR(320);
ALTER TABLE users ADD COLUMN is_blocked BOOLEAN NOT NULL DEFAULT FALSE;
ALTER TABLE users DROP COLUMN login;

INSERT INTO github_identities (github_id) VALUES (0);
UPDATE users SET github_id=0, email='mockuser@example.com' WHERE id='00000000-0000-0000-0000-000000000000';
