CREATE TABLE audit_logs (
    user_id uuid references users(id) NOT NULL,
    action text NOT NULL,
    meta jsonb NOT NULL,
    created_at timestamptz NOT NULL DEFAULT now()
);
