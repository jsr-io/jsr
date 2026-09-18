-- Bind an oauth_state to the user/session that initiated it. This is NULL for
-- the login flow (where there is no authenticated user yet) and set to the
-- current user for the account-linking ("connect") flow. The connect callback
-- requires this to match the authenticated user, which prevents a CSRF attack
-- where a victim is lured to the callback URL and gets an attacker's identity
-- linked to their account.
ALTER TABLE oauth_states
  ADD COLUMN user_id uuid REFERENCES users (id) ON DELETE CASCADE;
