// Copyright 2024 the JSR authors. All rights reserved. MIT license.
import { WebhookEndpoint, WebhookEventKind } from "../utils/api_types.ts";
import { useSignal } from "@preact/signals";
import { api, path } from "../utils/api.ts";
import { Help } from "../components/Help.tsx";

export const WEBHOOK_EVENTS: Array<{
  id: WebhookEventKind;
  name: string;
  description: string;
  packageLevel: boolean;
}> = [
  {
    id: "package_version_npm_tarball_ready",
    name: "Package version NPM tarball ready",
    description: "A NPM tarball for a published version is available.",
    packageLevel: true,
  },
  {
    id: "package_version_published",
    name: "Package version published",
    description: "A new version of a package is published.",
    packageLevel: true,
  },
  {
    id: "package_version_yanked",
    name: "Package version yanked",
    description: "A version of a package is yanked or unyanked.",
    packageLevel: true,
  },
  {
    id: "package_version_deleted",
    name: "Package version deleted",
    description: "A version of a package is deleted.",
    packageLevel: true,
  },
  {
    id: "scope_package_created",
    name: "Package created",
    description: "A new package is created in the scope.",
    packageLevel: false,
  },
  {
    id: "scope_package_deleted",
    name: "Package deleted",
    description: "A package is deleted in the scope.",
    packageLevel: false,
  },
  {
    id: "scope_package_archived",
    name: "Package archived",
    description: "A package in the scope is archived or unarchived.",
    packageLevel: false,
  },
  {
    id: "scope_member_added",
    name: "Scope member added",
    description: "A new member is added to the scope.",
    packageLevel: false,
  },
  {
    id: "scope_member_removed",
    name: "Scope member removed",
    description: "A member is removed from the scope.",
    packageLevel: false,
  },
];

function Required() {
  return <span class="text-red-500 text-sm align-text-top">*</span>;
}

export function WebhookEdit(
  { webhook, scope, package: pkg }: {
    scope: string;
    package?: string;
    webhook: WebhookEndpoint | null;
  },
) {
  const description = useSignal(webhook?.description ?? "");
  const url = useSignal(webhook?.url ?? "");
  const payloadFormat = useSignal(webhook?.payloadFormat ?? "json");
  const secret = useSignal<string | null>("");
  const clearSecret = useSignal(false);
  const events = useSignal(new Set(webhook?.events ?? []));
  const isActive = useSignal(webhook?.isActive ?? true);
  const processing = useSignal(false);
  const error = useSignal<string | null>(null);

  return (
    <form
      class="mt-8"
      autocomplete="off"
      onSubmit={(e) => {
        e.preventDefault();

        processing.value = true;
        error.value = null;

        (webhook
          ? api.patch(
            pkg
              ? path`/scopes/${scope}/packages/${pkg}/webhooks/${webhook.id}`
              : path`/scopes/${scope}/webhooks/${webhook.id}`,
            {
              description: description.value === webhook.description
                ? undefined
                : description.value,
              url: url.value === webhook.url ? undefined : url.value,
              payloadFormat: payloadFormat.value === webhook.payloadFormat
                ? undefined
                : payloadFormat.value,
              secret: secret.value || undefined,
              clearSecret: clearSecret.value || undefined,
              events: events.value.symmetricDifference(new Set(webhook.events))
                  .size === 0
                ? undefined
                : Array.from(events.value),
              isActive: isActive.value === webhook.isActive
                ? undefined
                : isActive.value,
            },
          )
          : api.post(
            pkg
              ? path`/scopes/${scope}/packages/${pkg}/webhooks`
              : path`/scopes/${scope}/webhooks`,
            {
              description: description.value,
              url: url.value,
              payloadFormat: payloadFormat.value,
              secret: secret.value || null,
              events: Array.from(events.value),
              isActive: isActive.value,
            },
          )).then((res) => {
            if (!res.ok) {
              error.value = res.message;
              processing.value = false;
              return;
            }
            if (webhook) {
              location.reload();
            } else {
              location.href = pkg
                ? `/@${scope}/${pkg}/settings#webhooks`
                : `/@${scope}/~/settings#webhooks`;
            }
          }).catch((err) => {
            error.value = String(err);
            processing.value = false;
          });
      }}
    >
      <div class="grid grid-cols-1 gap-5 md:grid-cols-2 md:gap-8">
        <div class="space-y-5">
          <label class="block">
            <h2 class="text-lg sm:text-xl font-semibold">Description</h2>
            <input
              type="text"
              class="inline-block w-full max-w-lg px-3 py-2 input-container text-sm input mt-3"
              placeholder="My webhook..."
              value={description}
              onInput={(e) => description.value = e.currentTarget.value}
              disabled={processing}
            />
          </label>
          <label class="block">
            <h2 class="text-lg sm:text-xl font-semibold">
              URL <Required />
            </h2>
            <input
              type="url"
              class="inline-block w-full max-w-lg px-3 py-2 input-container text-sm input mt-3"
              placeholder="https://example.com/webhook"
              value={url}
              onInput={(e) => url.value = e.currentTarget.value}
              required
              disabled={processing}
            />
          </label>
          <label class="block">
            <h2 class="text-lg sm:text-xl font-semibold">
              Payload format <Required />{" "}
              <Help href="/docs/webhooks#payload-format" />
            </h2>
            <select
              name="payload_format"
              className="input-container input select w-full max-w-lg block px-3 py-2 text-sm mt-3"
              required
              disabled={processing}
              onChange={(e) =>
                payloadFormat.value = e.currentTarget
                  .value as WebhookEndpoint["payloadFormat"]}
            >
              <option value="json" selected={payloadFormat.value === "json"}>
                JSON
              </option>
              <option
                value="discord"
                selected={payloadFormat.value === "discord"}
              >
                Discord
              </option>
              <option
                value="slack"
                selected={payloadFormat.value === "slack"}
              >
                Slack
              </option>
            </select>
          </label>
          <label class="block">
            <h2 class="text-lg sm:text-xl font-semibold">
              Secret{" "}
              <Help href="/docs/webhooks#secrets-and-signature-verification" />
            </h2>
            {webhook?.hasSecret && (
              clearSecret.value
                ? (
                  <div class="text-sm text-secondary">
                    Secret will be cleared on save.{" "}
                    <button
                      type="button"
                      class="underline"
                      onClick={() => clearSecret.value = false}
                    >
                      Undo
                    </button>
                  </div>
                )
                : (
                  <div class="text-sm text-red-500">
                    A secret is already set. Inputting a new value will
                    overwrite the current secret.
                  </div>
                )
            )}
            <div class="flex justify-between gap-3 max-w-lg mt-3">
              <input
                type="text"
                class="inline-block w-full px-3 py-2 input-container text-sm input"
                value={secret.value ?? ""}
                onInput={(e) => {
                  secret.value = e.currentTarget.value;
                  clearSecret.value = false;
                }}
                disabled={processing.value || clearSecret.value}
              />
              {webhook?.hasSecret && !clearSecret.value && (
                <button
                  class="button-danger"
                  type="button"
                  onClick={() => {
                    clearSecret.value = true;
                    secret.value = "";
                  }}
                  disabled={processing}
                >
                  Clear
                </button>
              )}
            </div>
          </label>
        </div>
        <fieldset>
          <legend>
            <h2 class="text-lg sm:text-xl font-semibold">
              Events
            </h2>
          </legend>
          <div class="grid grid-cols-1 md:grid-cols-2 gap-4 mt-3">
            {WEBHOOK_EVENTS.filter((event) => {
              if (pkg) {
                return event.packageLevel;
              } else {
                return true;
              }
            }).map((event) => (
              <label key={event.id} class="block pl-6">
                <input
                  type="checkbox"
                  class="-ml-6 mt-1.5 float-left"
                  name="events"
                  value={event.id}
                  checked={events.value.has(event.id)}
                  onInput={(e) => {
                    const next = new Set(events.value);
                    if (e.currentTarget.checked) {
                      next.add(event.id);
                    } else {
                      next.delete(event.id);
                    }
                    events.value = next;
                  }}
                  disabled={processing}
                />
                <h3 class="sm:text-lg font-medium inline-block">
                  {event.name}
                </h3>
                <div class="text-sm">{event.description}</div>
              </label>
            ))}
          </div>
        </fieldset>
      </div>
      {error.value && (
        <div class="mt-4 p-3 bg-red-50 dark:bg-red-950 border border-red-200 dark:border-red-800 rounded text-red-700 dark:text-red-300 text-sm">
          {error.value}
        </div>
      )}
      <div class="flex gap-8 mt-8 items-center">
        {webhook && (
          <button
            type="button"
            class="button-danger"
            disabled={processing}
            onClick={() => {
              if (!confirm("Are you sure you want to delete this webhook?")) {
                return;
              }
              processing.value = true;

              api.delete(
                pkg
                  ? path`/scopes/${scope}/packages/${pkg}/webhooks/${webhook.id}`
                  : path`/scopes/${scope}/webhooks/${webhook.id}`,
              ).then((res) => {
                if (!res.ok) {
                  error.value = res.message;
                  processing.value = false;
                  return;
                }
                location.href = pkg
                  ? `/@${scope}/${pkg}/settings#webhooks`
                  : `/@${scope}/~/settings#webhooks`;
              }).catch((err) => {
                error.value = String(err);
                processing.value = false;
              });
            }}
          >
            Delete
          </button>
        )}
        <button type="submit" class="button-primary" disabled={processing}>
          {webhook ? "Save" : "Create"}
        </button>
        <label class="block pl-6 ml-8">
          <input
            type="checkbox"
            class="-ml-6 mt-1.5 float-left"
            checked={isActive}
            onInput={(e) => isActive.value = e.currentTarget.checked}
            disabled={processing}
          />
          <h2 class="sm:text-lg font-medium inline-block">Active</h2>
        </label>
      </div>
    </form>
  );
}
