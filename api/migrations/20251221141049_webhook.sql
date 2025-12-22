CREATE TYPE webhook_event_kind AS ENUM (
    'package_version_published',
    'package_version_yanked',
    'package_version_deleted',
    'scope_package_created',
    'scope_package_archived',
    'scope_member_added',
    'scope_member_left'
);

CREATE TYPE webhook_payload_format AS ENUM (
    'json',
    'discord'
);

CREATE TABLE webhook_endpoints (
    id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    scope TEXT NOT NULL references scopes (scope),
    package TEXT,
    url TEXT NOT NULL,
    description TEXT,
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
    scope TEXT NOT NULL references scopes (scope),
    package TEXT,
    event webhook_event_kind NOT NULL,
    payload JSONB NOT NULL,
    idempotency_key TEXT UNIQUE,
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
    status webhook_delivery_status NOT NULL,

    request_headers JSONB,

    response_http_code INT,
    response_headers JSONB,
    response_body TEXT,

    updated_at timestamptz NOT NULL DEFAULT now(),
    created_at timestamptz NOT NULL DEFAULT now()
);
SELECT manage_updated_at('webhook_deliveries');
