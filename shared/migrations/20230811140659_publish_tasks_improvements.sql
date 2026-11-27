DROP TABLE publishing_tasks;
CREATE TABLE publishing_tasks (
  id uuid PRIMARY KEY DEFAULT uuid_generate_v4(),

  status task_status NOT NULL DEFAULT 'pending',
  error jsonb, -- only set if status is 'failure'

  user_id uuid NOT NULL REFERENCES users (id),

  package_scope text NOT NULL REFERENCES scopes (scope),
  package_name text NOT NULL,
  package_version text NOT NULL,

  created_at timestamptz NOT NULL DEFAULT now(),
  updated_at timestamptz NOT NULL DEFAULT now(),

  FOREIGN KEY (package_scope, package_name) REFERENCES packages (scope, name)
);
SELECT manage_updated_at('publishing_tasks');

CREATE INDEX idx_publishing_tasks_scope_package_version ON publishing_tasks (package_scope, package_name, package_version);