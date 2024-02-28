ALTER TABLE scope_invites DROP CONSTRAINT scope_invites_scope_target_user_id_requesting_user_id_key;
ALTER TABLE scope_invites ADD UNIQUE (scope, target_user_id);
