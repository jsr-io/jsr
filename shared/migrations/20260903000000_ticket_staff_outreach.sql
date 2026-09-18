-- Staff can open a ticket on a user's behalf to start a conversation with them.
-- The ticket is owned by the user it is addressed to, so it shows up alongside
-- their own tickets and their replies count as inbound; the first message is
-- outbound, written by the staff member who opened it.
ALTER TYPE ticket_kind ADD VALUE 'staff_outreach';
