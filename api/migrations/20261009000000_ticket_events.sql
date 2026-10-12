-- Things that happen to a ticket other than a message being written: its status
-- changing, or an email-opened ticket being claimed by an account. These used
-- to be read out of audit_logs by pattern-matching meta as text, which scans the
-- whole audit log on every ticket view.
CREATE TYPE ticket_event_kind AS ENUM ('status_change', 'claimed');

CREATE TABLE ticket_events (
    id uuid PRIMARY KEY DEFAULT gen_random_uuid(),
    ticket_id uuid NOT NULL REFERENCES tickets(id) ON DELETE CASCADE,
    actor uuid NOT NULL REFERENCES users(id),
    kind ticket_event_kind NOT NULL,
    -- The status moved to, for a status change.
    status ticket_status,
    created_at timestamptz NOT NULL DEFAULT now(),
    CONSTRAINT ticket_events_status_iff_status_change CHECK (
        (kind = 'status_change') = (status IS NOT NULL)
    )
);

CREATE INDEX ticket_events_ticket_id_idx ON ticket_events (ticket_id, created_at);
CREATE INDEX ticket_events_actor_idx ON ticket_events (actor);

-- Carry over the status changes already recorded in the audit log. Entries
-- written before the status enum recorded a `closed` boolean instead.
INSERT INTO ticket_events (ticket_id, actor, kind, status, created_at)
SELECT
    tickets.id,
    audit_logs.actor_id,
    'status_change',
    COALESCE(
        (audit_logs.meta->>'status')::ticket_status,
        CASE WHEN (audit_logs.meta->>'closed')::boolean THEN 'closed' ELSE 'open' END::ticket_status
    ),
    audit_logs.created_at
FROM audit_logs
INNER JOIN tickets ON tickets.id::text = audit_logs.meta->>'ticket_id'
WHERE audit_logs.action = 'update_ticket_status'
    AND (audit_logs.meta ? 'status' OR audit_logs.meta ? 'closed');
