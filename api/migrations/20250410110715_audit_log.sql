CREATE TABLE audit_logs (
    actor_id uuid references users(id) NOT NULL,
    is_sudo bool NOT NULL,
    action text NOT NULL,
    meta jsonb NOT NULL,
    created_at timestamptz NOT NULL DEFAULT now()
);
