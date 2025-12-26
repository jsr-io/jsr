import { WebhookEndpoint, WebhookEventKind } from "../utils/api_types.ts";

const events: Array<{
  id: WebhookEventKind;
  name: string;
  description: string;
  packageLevel: boolean;
}> = [
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
  { webhook, packageLevel }: {
    webhook: WebhookEndpoint | null;
    packageLevel?: boolean;
  },
) {
  return (
    <form class="mt-8" autocomplete="off">
      <div class="grid grid-cols-1 gap-5 md:grid-cols-2 md:gap-8">
        <div class="space-y-5">
          <label class="block">
            <h2 class="text-lg sm:text-xl font-semibold">Description</h2>
            <input
              type="text"
              class="inline-block w-full max-w-lg px-3 py-2 input-container text-sm input mt-3"
              placeholder="My webhook..."
              value={webhook?.description ?? ""}
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
              value={webhook?.url}
              required
            />
          </label>
          <label class="block">
            <h2 class="text-lg sm:text-xl font-semibold">
              Payload format <Required />
            </h2>
            <select
              name="payload_format"
              className="input-container input select w-full max-w-lg block px-3 py-2 text-sm mt-3"
              required
            >
              <option value="json" selected={webhook?.payloadFormat === "json"}>
                JSON
              </option>
              <option
                value="discord"
                selected={webhook?.payloadFormat === "discord"}
              >
                Discord
              </option>
              <option
                value="slack"
                selected={webhook?.payloadFormat === "slack"}
              >
                Slack
              </option>
            </select>
          </label>
          <label class="block">
            <h2 class="text-lg sm:text-xl font-semibold">Secret</h2>
            {webhook?.hasSecret && (
              <div class="text-sm text-red-500">
                A secret is already set. Inputting a new value will overwrite
                the current secret
              </div>
            )}
            <input
              type="text"
              class="inline-block w-full max-w-lg px-3 py-2 input-container text-sm input mt-3"
            />
          </label>
        </div>
        <fieldset>
          <legend>
            <h2 class="text-lg sm:text-xl font-semibold">
              Events <Required />
            </h2>
          </legend>
          <div class="grid grid-cols-1 md:grid-cols-2 gap-4 mt-3">
            {events.filter((event) => {
              if (packageLevel) {
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
                  checked={webhook?.events.includes(event.id)}
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
      <div class="flex gap-8 mt-8 items-center">
        {webhook && <button type="submit" class="button-danger">Delete</button>}
        <button type="submit" class="button-primary">
          {webhook ? "Save" : "Create"}
        </button>
        <label class="block pl-6 ml-8">
          <input
            type="checkbox"
            class="-ml-6 mt-1.5 float-left"
            checked={webhook?.isActive ?? true}
          />
          <h2 class="sm:text-lg font-medium inline-block">Active</h2>
        </label>
      </div>
    </form>
  );
}
