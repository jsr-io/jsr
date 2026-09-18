-- Staff notes on a ticket: written in the ticket, never emailed, never shown to
-- the person who opened it.
--
-- Modelled as a flag on the existing messages rather than a table of its own so
-- that a note keeps its place in the conversation — the point of writing one is
-- that it sits next to the message it is about.

ALTER TABLE ticket_messages ADD COLUMN internal bool NOT NULL DEFAULT false;

-- A note is written in the JSR interface by a signed-in staff member, and is
-- never sent anywhere, so it can carry no email identity of its own.
ALTER TABLE ticket_messages ADD CONSTRAINT ticket_messages_internal_is_unsent_and_authored CHECK (
    internal = false OR (author IS NOT NULL AND direction = 'outbound' AND email_message_id IS NULL)
);

-- The ticket page loads a whole conversation at a time, and every such read now
-- filters on this, so it belongs in the index that serves them.
DROP INDEX idx_ticket_messages_ticket_id;
CREATE INDEX idx_ticket_messages_ticket_id ON ticket_messages (ticket_id, created_at) INCLUDE (internal);
