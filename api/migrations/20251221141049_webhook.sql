CREATE TYPE webhook_event_kind AS ENUM (
    'package_version_npm_tarball_ready',
    'package_version_published',
    'package_version_yanked',
    'package_version_deleted',
    'scope_package_created',
    'scope_package_deleted',
    'scope_package_archived',
    'scope_member_added',
    'scope_member_removed'
);

CREATE TYPE webhook_payload_format AS ENUM (
    'json',
    'discord',
    'slack'
);

CREATE TABLE webhook_endpoints (
    id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    scope TEXT NOT NULL references scopes (scope) ON DELETE CASCADE,
    package TEXT,
    url TEXT NOT NULL,
    description TEXT NOT NULL DEFAULT '',
    secret VARCHAR(255),
    events webhook_event_kind[] NOT NULL,
    payload_format webhook_payload_format NOT NULL,
    is_active BOOLEAN NOT NULL DEFAULT TRUE,
    updated_at timestamptz NOT NULL DEFAULT now(),
    created_at timestamptz NOT NULL DEFAULT now(),

    FOREIGN KEY (scope, package) REFERENCES packages (scope, name) ON DELETE CASCADE
);

SELECT manage_updated_at('webhook_endpoints');

CREATE TABLE webhook_events (
    id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    scope TEXT NOT NULL references scopes (scope) ON DELETE CASCADE,
    package TEXT,
    event webhook_event_kind NOT NULL,
    payload JSONB NOT NULL,
    created_at timestamptz NOT NULL DEFAULT now(),

    FOREIGN KEY (scope, package) REFERENCES packages (scope, name) ON DELETE CASCADE
);

CREATE TYPE webhook_delivery_status AS ENUM (
    'pending', 'success', 'failure', 'retrying'
);

CREATE TABLE webhook_deliveries (
    id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    endpoint_id UUID NOT NULL REFERENCES webhook_endpoints (id) ON DELETE CASCADE,
    event_id UUID NOT NULL REFERENCES webhook_events(id) ON DELETE CASCADE,
    status webhook_delivery_status NOT NULL DEFAULT 'pending',

    request_headers JSONB,
    request_body JSONB,

    response_http_code INT,
    response_headers JSONB,
    response_body TEXT,

    error TEXT,

    updated_at timestamptz NOT NULL DEFAULT now(),
    created_at timestamptz NOT NULL DEFAULT now()
);
SELECT manage_updated_at('webhook_deliveries');

-- Indexes for webhook_endpoints lookup during fan-out
CREATE INDEX idx_webhook_endpoints_scope_active ON webhook_endpoints (scope, is_active) WHERE is_active = TRUE;
CREATE INDEX idx_webhook_endpoints_scope_package_active ON webhook_endpoints (scope, package, is_active) WHERE is_active = TRUE;

-- Index for listing events by scope/package
CREATE INDEX idx_webhook_events_scope ON webhook_events (scope, package, created_at DESC);

-- Index for listing deliveries by endpoint
CREATE INDEX idx_webhook_deliveries_endpoint ON webhook_deliveries (endpoint_id, created_at DESC);

-- Index for finding pending deliveries to dispatch
CREATE INDEX idx_webhook_deliveries_pending ON webhook_deliveries (status) WHERE status = 'pending';
