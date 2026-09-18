ALTER TABLE scope_invites ADD UNIQUE (scope, target_user_id, requesting_user_id);
