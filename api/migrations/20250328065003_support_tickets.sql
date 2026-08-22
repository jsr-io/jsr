CREATE TYPE ticket_kind AS ENUM ('user_scope_quota_increase', 'scope_quota_increase', 'scope_claim', 'package_report', 'other');

CREATE TABLE tickets (
    id uuid NOT NULL PRIMARY KEY DEFAULT uuid_generate_v4(),
    kind ticket_kind NOT NULL,
    creator uuid NOT NULL REFERENCES users(id),
    meta jsonb NOT NULL DEFAULT '{}'::jsonb,
    closed bool NOT NULL DEFAULT false,
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);
SELECT manage_updated_at('tickets');

CREATE TABLE ticket_messages (
    ticket_id uuid NOT NULL REFERENCES tickets(id),
    author uuid NOT NULL REFERENCES users(id),
    message text NOT NULL,
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);
SELECT manage_updated_at('ticket_messages');
