-- Deletion hold: while set, the account cannot be deleted (neither
-- self-service nor via the admin API) until the hold is lifted. This exists
-- to satisfy evidence-preservation obligations: once litigation is reasonably
-- anticipated (e.g. a cease & desist involving the user), deleting the
-- account's identifying data could constitute spoliation. GDPR art. 17(3)(b)/(e)
-- and CCPA 1798.105(d) permit refusing erasure in these circumstances.
ALTER TABLE users ADD COLUMN deletion_hold boolean NOT NULL DEFAULT false;
