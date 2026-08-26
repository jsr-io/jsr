-- Outbox for outgoing email. Sending used to happen inline in the request, so a
-- Postmark hiccup failed the user's request with the work already committed —
-- an admin's ticket reply would be saved but return 500, and retyping it left
-- two copies on the ticket. Rows here are handed to Cloud Tasks, which retries
-- delivery independently of the request that queued it.
--
-- The rendered subject and bodies are stored rather than the arguments they were
-- rendered from: a delivery then cannot be changed out from under itself by a
-- later template edit, and the row says exactly what was sent.

CREATE TABLE email_deliveries (
    id uuid NOT NULL PRIMARY KEY DEFAULT uuid_generate_v4(),
    to_address text NOT NULL,
    subject text NOT NULL,
    body_text text NOT NULL,
    body_html text NOT NULL,

    -- RFC 5322 threading, angle brackets included. Sending the same row twice
    -- reuses the same Message-ID, so a duplicate delivery is collapsed by the
    -- recipient's mail client rather than showing up as a second email.
    message_id text,
    in_reply_to text,
    reference_ids text[] NOT NULL DEFAULT '{}',

    attempts int NOT NULL DEFAULT 0,
    -- Set once Postmark has accepted the message; the row is never sent again.
    sent_at TIMESTAMPTZ,
    -- Set when the delivery has failed too many times to keep retrying. Kept
    -- rather than deleted so a human can see what was lost and why.
    abandoned_at TIMESTAMPTZ,
    last_error text,

    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);
SELECT manage_updated_at('email_deliveries');

-- Drives the sweeper, which looks for deliveries that were queued but never
-- reached a terminal state.
CREATE INDEX idx_email_deliveries_pending ON email_deliveries (created_at)
    WHERE sent_at IS NULL AND abandoned_at IS NULL;
