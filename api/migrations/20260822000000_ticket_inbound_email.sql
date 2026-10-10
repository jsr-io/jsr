-- Support tickets can now originate from inbound email, so a ticket no longer
-- necessarily has a JSR user behind it, and a message no longer necessarily has
-- a JSR user as its author. See api/src/api/hooks.rs.

CREATE TYPE ticket_status AS ENUM ('open', 'waiting_on_user', 'waiting_on_support', 'closed', 'spam');
CREATE TYPE ticket_message_direction AS ENUM ('inbound', 'outbound');

-- Human-quotable ticket identifier, safe to put in an email subject line. The
-- loop retries on the (vanishingly unlikely) collision rather than relying on
-- the unique index to surface it as an error.
CREATE FUNCTION generate_ticket_number() RETURNS text AS $$
DECLARE
  candidate text;
  taken boolean;
BEGIN
  LOOP
    candidate := 'TICKET-' || TO_CHAR(NOW(), 'YYYYMMDD') || '-' ||
                 LPAD(FLOOR(RANDOM() * 100000)::text, 5, '0');
    SELECT EXISTS(SELECT 1 FROM tickets WHERE ticket_number = candidate) INTO taken;
    EXIT WHEN NOT taken;
  END LOOP;
  RETURN candidate;
END;
$$ LANGUAGE plpgsql;

ALTER TABLE tickets
    ADD COLUMN ticket_number text UNIQUE,
    ADD COLUMN status ticket_status NOT NULL DEFAULT 'open',
    ADD COLUMN claim_token uuid UNIQUE,
    ADD COLUMN reporter_email text,
    ADD COLUMN reporter_name text,
    ADD COLUMN subject text,
    ADD COLUMN closed_at TIMESTAMPTZ;

-- Backfill in two steps: the DEFAULT cannot be attached until every existing row
-- has a number, because generate_ticket_number() reads the column it fills.
UPDATE tickets SET ticket_number = generate_ticket_number();
ALTER TABLE tickets
    ALTER COLUMN ticket_number SET NOT NULL,
    ALTER COLUMN ticket_number SET DEFAULT generate_ticket_number();

UPDATE tickets SET status = 'closed', closed_at = updated_at WHERE closed = true;

-- Must go before the column drop, which would otherwise take this partial index
-- (predicated on `closed`) down with it.
DROP INDEX idx_tickets_open_by_creator;

ALTER TABLE tickets
    ALTER COLUMN creator DROP NOT NULL,
    DROP COLUMN closed;

-- A ticket is owned either by a JSR user or by an email address, never both and
-- never neither. Claiming a ticket moves it from the second form to the first.
ALTER TABLE tickets ADD CONSTRAINT tickets_reporter_xor CHECK (
    (creator IS NOT NULL AND reporter_email IS NULL AND reporter_name IS NULL) OR
    (creator IS NULL AND reporter_email IS NOT NULL)
);

CREATE INDEX idx_tickets_open_by_creator ON tickets (creator) WHERE status <> 'closed' AND status <> 'spam';
CREATE INDEX idx_tickets_status ON tickets (status);

-- ticket_messages had no primary key; ticket_attachments needs one to point at.
ALTER TABLE ticket_messages
    ADD COLUMN id uuid NOT NULL PRIMARY KEY DEFAULT uuid_generate_v4(),
    ADD COLUMN direction ticket_message_direction,
    ADD COLUMN author_email text,
    ADD COLUMN author_name text,
    ADD COLUMN author_email_verified bool,
    ADD COLUMN email_message_id text UNIQUE;

-- Pre-existing messages were all written through the web UI: the ticket creator
-- speaking is inbound, anyone else (staff) is outbound.
UPDATE ticket_messages SET direction = (
    CASE WHEN author = (SELECT creator FROM tickets WHERE tickets.id = ticket_messages.ticket_id)
        THEN 'inbound'::ticket_message_direction
        ELSE 'outbound'::ticket_message_direction
    END
);
ALTER TABLE ticket_messages
    ALTER COLUMN direction SET NOT NULL,
    ALTER COLUMN author DROP NOT NULL;

-- A message is attributed either to a JSR user or to an email address. The
-- system's own auto-reply is attributed to neither.
ALTER TABLE ticket_messages ADD CONSTRAINT ticket_messages_author_xor CHECK (
    (author IS NOT NULL AND author_email IS NULL AND author_name IS NULL AND author_email_verified IS NULL) OR
    (author IS NULL AND author_email IS NOT NULL AND author_email_verified IS NOT NULL) OR
    (author IS NULL AND author_email IS NULL AND author_name IS NULL AND author_email_verified IS NULL AND direction = 'outbound')
);

-- Anonymous authorship only makes sense for mail we received.
ALTER TABLE ticket_messages ADD CONSTRAINT ticket_messages_anonymous_author_inbound_only CHECK (
    author_email IS NULL OR direction = 'inbound'
);

CREATE TABLE ticket_attachments (
    id uuid NOT NULL PRIMARY KEY DEFAULT uuid_generate_v4(),
    message_id uuid NOT NULL REFERENCES ticket_messages(id) ON DELETE CASCADE,
    filename text NOT NULL,
    content_type text NOT NULL,
    size_bytes int NOT NULL,
    storage_key text NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX idx_ticket_attachments_message_id ON ticket_attachments (message_id);
