-- Add proper ON DELETE behavior to foreign keys referencing users(id)
-- so that deleting a user doesn't require manual cleanup of every table.

-- scopes.creator: change from CASCADE (would delete scopes!) to SET NULL
ALTER TABLE scopes DROP CONSTRAINT scopes_creator_fkey;
ALTER TABLE scopes ADD CONSTRAINT scopes_creator_fkey
    FOREIGN KEY (creator) REFERENCES users(id) ON DELETE SET NULL;

-- scope_members: remove memberships when user is deleted
ALTER TABLE scope_members DROP CONSTRAINT scope_members_user_id_fkey;
ALTER TABLE scope_members ADD CONSTRAINT scope_members_user_id2_fkey
    FOREIGN KEY (user_id) REFERENCES users(id) ON DELETE CASCADE;

-- scope_invites: remove invites when user is deleted
ALTER TABLE scope_invites DROP CONSTRAINT scope_invites_target_user_id_fkey;
ALTER TABLE scope_invites ADD CONSTRAINT scope_invites_target_user_id_fkey
    FOREIGN KEY (target_user_id) REFERENCES users(id) ON DELETE CASCADE;

ALTER TABLE scope_invites DROP CONSTRAINT scope_invites_requesting_user_id_fkey;
ALTER TABLE scope_invites ADD CONSTRAINT scope_invites_requesting_user_id_fkey
    FOREIGN KEY (requesting_user_id) REFERENCES users(id) ON DELETE CASCADE;

-- publishing_tasks: make nullable, set null on delete
ALTER TABLE publishing_tasks ALTER COLUMN user_id DROP NOT NULL;
ALTER TABLE publishing_tasks DROP CONSTRAINT publishing_tasks_user_id_fkey;
ALTER TABLE publishing_tasks ADD CONSTRAINT publishing_tasks_user_id_fkey
    FOREIGN KEY (user_id) REFERENCES users(id) ON DELETE SET NULL;

-- package_versions: already nullable, just add cascade behavior
ALTER TABLE package_versions DROP CONSTRAINT IF EXISTS package_versions_user_id_fkey;
ALTER TABLE package_versions ADD CONSTRAINT package_versions_user_id_fkey
    FOREIGN KEY (user_id) REFERENCES users(id) ON DELETE SET NULL;
