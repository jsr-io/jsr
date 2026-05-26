---
title: Webhooks
description: Webhooks allow you to receive HTTP notifications when events occur in your scope or packages.
---

Webhooks allow you to receive real-time HTTP notifications when events occur in
your JSR scope or packages. You can use webhooks to trigger CI/CD pipelines,
send notifications to chat services, or integrate with other tools.

## Creating a webhook

Webhooks can be created at two levels:

- **Scope-level webhooks**: Receive notifications for all events in a scope and
  its packages.
- **Package-level webhooks**: Receive notifications only for events related to a
  specific package.

To create a webhook:

1. Navigate to your scope or package settings
2. Go to the "Webhooks" section
3. Click "Create webhook"
4. Configure the webhook URL, events, and optional secret

## Events

### Package events

These events are triggered for specific packages. For scope-level webhooks,
these events are triggered for all packages in the scope.

| Event                               | Description                                       |
| ----------------------------------- | ------------------------------------------------- |
| `package_version_published`         | A new version of a package was published          |
| `package_version_yanked`            | A package version was yanked or unyanked          |
| `package_version_deleted`           | A package version was deleted                     |
| `package_version_npm_tarball_ready` | The npm-compatible tarball for a version is ready |

### Scope events

These events are triggered at the scope level and are only available for
scope-level webhooks.

| Event                    | Description                            |
| ------------------------ | -------------------------------------- |
| `scope_package_created`  | A new package was created in the scope |
| `scope_package_deleted`  | A package was deleted from the scope   |
| `scope_package_archived` | A package was archived or unarchived   |
| `scope_member_added`     | A new member was added to the scope    |
| `scope_member_removed`   | A member was removed from the scope    |

## Payload format

Webhooks support three payload formats:

- **JSON**: Standard JSON payload
- **Discord**: Pre-formatted for Discord webhook endpoints
- **Slack**: Pre-formatted for Slack webhook endpoints

### JSON payloads

All JSON payloads include an `event` field that identifies the event type. The
remaining fields vary by event.

#### `package_version_published`

```json
{
  "event": "package_version_published",
  "scope": "myorg",
  "package": "mylib",
  "version": "1.0.0",
  "user_id": "550e8400-e29b-41d4-a716-446655440000"
}
```

The `user_id` field contains the UUID of the user who published the version, or
`null` if published via CI without user context.

#### `package_version_yanked`

```json
{
  "event": "package_version_yanked",
  "scope": "myorg",
  "package": "mylib",
  "version": "1.0.0",
  "yanked": true
}
```

The `yanked` field is `true` when a version is yanked, and `false` when a
version is unyanked.

#### `package_version_deleted`

```json
{
  "event": "package_version_deleted",
  "scope": "myorg",
  "package": "mylib",
  "version": "1.0.0"
}
```

#### `package_version_npm_tarball_ready`

```json
{
  "event": "package_version_npm_tarball_ready",
  "scope": "myorg",
  "package": "mylib",
  "version": "1.0.0"
}
```

This event is triggered after a version is published and the npm-compatible
tarball has been built. Use this event if you need to wait for npm compatibility
before taking action.

#### `scope_package_created`

```json
{
  "event": "scope_package_created",
  "scope": "myorg",
  "package": "mylib"
}
```

#### `scope_package_deleted`

```json
{
  "event": "scope_package_deleted",
  "scope": "myorg",
  "package": "mylib"
}
```

#### `scope_package_archived`

```json
{
  "event": "scope_package_archived",
  "scope": "myorg",
  "package": "mylib",
  "archived": true
}
```

The `archived` field is `true` when a package is archived, and `false` when a
package is unarchived.

#### `scope_member_added`

```json
{
  "event": "scope_member_added",
  "scope": "myorg",
  "user_id": "550e8400-e29b-41d4-a716-446655440000"
}
```

#### `scope_member_removed`

```json
{
  "event": "scope_member_removed",
  "scope": "myorg",
  "user_id": "550e8400-e29b-41d4-a716-446655440000"
}
```

## HTTP headers

Each webhook request includes the following HTTP headers:

| Header            | Description                                           |
| ----------------- | ----------------------------------------------------- |
| `X-JSR-Event`     | The event type (e.g., `"package_version_published"`)  |
| `X-JSR-Event-Id`  | Unique identifier for this event                      |
| `X-JSR-Signature` | HMAC signature of the request body (if secret is set) |

## Secrets and signature verification

Webhook secrets allow you to verify that incoming webhook requests genuinely
originate from JSR and have not been tampered with in transit.

### How secrets work

When you configure a secret for your webhook, JSR computes an HMAC-SHA256
signature of the request body using your secret which in the `X-JSR-Signature`
header. Your server can verify this signature to validate that the request is
genuine.

The signature format is: `sha256=<hex-encoded-signature>`

### Verifying signatures

To verify a webhook signature:

1. Extract the signature from the `X-JSR-Signature` header
2. Compute an HMAC-SHA256 hash of the raw request body using your secret
3. Compare the computed hash with the signature from the header

## Delivery and retries

JSR delivers webhooks with the following behavior:

- Webhooks are delivered asynchronously after events occur
- Failed deliveries (non-2xx responses) are retried up to 3 times
- You can view delivery history and debug failed deliveries in the webhook
  settings
