-- Index on scopes.creator for COUNT subqueries calculating scope_usage
-- Used in: user queries that calculate "scope_usage" (how many scopes a user created)
CREATE INDEX idx_scopes_creator ON scopes (creator);

-- Index on scope_invites.target_user_id for invite lookups and COUNT subqueries
-- Used in: invite_count calculations and scope invite queries by target user
CREATE INDEX idx_scope_invites_target_user_id ON scope_invites (target_user_id);

-- Index on tickets.creator for user's tickets queries
-- Used in: listing tickets by creator, COUNT subqueries for unread ticket counts
CREATE INDEX idx_tickets_creator ON tickets (creator);

-- Partial index on open tickets by creator (most common filter pattern)
-- Used in: counting open tickets with unread messages for a user
CREATE INDEX idx_tickets_open_by_creator ON tickets (creator) WHERE closed = false;

-- Index on ticket_messages for retrieving messages by ticket ordered by time
-- Used in: all ticket message retrieval queries
CREATE INDEX idx_ticket_messages_ticket_id ON ticket_messages (ticket_id, created_at);

-- Composite index for "latest version" lookups (extremely common pattern)
-- Used in: nearly every package query that needs the latest non-prerelease, non-yanked version
-- The WHERE clause pattern is: scope = $1 AND name = $2 AND version NOT LIKE '%-%' AND is_yanked = false ORDER BY version DESC LIMIT 1
CREATE INDEX idx_package_versions_latest ON package_versions (scope, name, is_yanked, version DESC)
  WHERE is_yanked = false AND version NOT LIKE '%-%';

-- Index for newest packages queries
-- Used in: "recently added" package listings
CREATE INDEX idx_packages_created_at ON packages (created_at DESC);

-- Index for featured packages queries
-- Used in: homepage featured packages listing
CREATE INDEX idx_packages_when_featured ON packages (when_featured DESC NULLS LAST)
  WHERE when_featured IS NOT NULL;

-- Index for publishing rate limit checks
-- Used in: COUNT queries for publish_attempts_per_week (package_scope + recent created_at)
CREATE INDEX idx_publishing_tasks_scope_created_at ON publishing_tasks (package_scope, created_at DESC);

-- Partial index for active (non-failed) publishing tasks
-- Used in: checking if there's an active publishing task for a package
CREATE INDEX idx_publishing_tasks_active ON publishing_tasks (package_scope, package_name)
  WHERE status != 'failure';
