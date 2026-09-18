CREATE TABLE oauth_device_states (
  id uuid NOT NULL PRIMARY KEY DEFAULT uuid_generate_v4(),
  auth text NOT NULL
);
SELECT manage_updated_at('oauth_device_states');
