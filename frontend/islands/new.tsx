// Copyright 2024 the JSR authors. All rights reserved. MIT license.
import {
  Signal,
  useComputed,
  useSignal,
  useSignalEffect,
} from "@preact/signals";
import { ComponentChildren } from "preact";
import twas from "twas";
import { api, path } from "../utils/api.ts";
import {
  validatePackageName,
  validateScopeDescription,
  validateScopeName,
} from "../utils/ids.ts";
import { TicketModal } from "./TicketModal.tsx";
import type { Package, Scope, User } from "../utils/api_types.ts";
interface IconColorProps {
  done: Signal<unknown>;
  children: ComponentChildren;
}

export function IconCircle({ done, children }: IconColorProps) {
  const color = useComputed(() => {
    if (done.value) return "bg-green-600 text-white";
    return "bg-jsr-gray-100 dark:bg-jsr-gray-900 text-black dark:text-jsr-gray-200";
  });
  return (
    <div class={color + " hidden md:block rounded-full p-1.75"}>
      {children}
    </div>
  );
}

interface ScopeSelectProps {
  scopes: string[];
  scope: Signal<string>;
  initialScope: string | undefined;
  scopeUsage: number;
  scopeLimit: number;
  locked: boolean;
  user: User;
}

export function ScopeSelect(
  {
    scopes: initialScopes,
    scope,
    initialScope,
    scopeLimit: initialScopeLimit,
    scopeUsage: initialScopeUsage,
    locked,
    user,
  }: ScopeSelectProps,
) {
  const scopeUsage = useSignal(initialScopeUsage);
  const scopeLimit = useSignal(initialScopeLimit);

  const scopes = useSignal(initialScopes);
  const explicitCreateScope = useSignal(initialScope !== undefined);

  if (scopes.value.length === 0) {
    return (
      <div class="space-y-4 bg-jsr-cyan-50 border-1.5 border-jsr-cyan-200 dark:bg-jsr-cyan-950 dark:border-jsr-cyan-700 p-4 md:p-6 rounded-xl">
        <span class="text-jsr-gray-700 dark:text-white">
          You are not a member of any scopes. Create a new scope to publish your
          package.
        </span>
        <CreateScope
          initialValue={initialScope}
          onCreate={(newScope) => {
            scopes.value = [...scopes.value, newScope];
            scope.value = newScope;
          }}
          locked={locked}
          user={user}
        />
      </div>
    );
  }

  if (explicitCreateScope.value) {
    const scopesLeft = scopeLimit.value - scopeUsage.value;

    return (
      <>
        <CreateScope
          initialValue={initialScope}
          onCreate={(newScope) => {
            scopes.value = [...scopes.value, newScope];
            explicitCreateScope.value = false;
            scope.value = newScope;
            scopeUsage.value++;
          }}
          locked={locked}
          user={user}
        />
        {!locked && (
          <p class="mt-2">
            or{" "}
            <button
              type="button"
              class="inline link"
              onClick={() => explicitCreateScope.value = false}
            >
              select an existing scope
            </button>
          </p>
        )}
        <p class="text-jsr-gray-700 dark:text-jsr-gray-200 text-sm mt-2">
          You can create {scopesLeft === 0 ? "no" : scopesLeft}{" "}
          more scope{scopesLeft !== 1 && "s"}.{" "}
          <a href="/account/settings" class="link">View quotas</a> or{" "}
          <a href="/account" class="link">manage your scopes</a>.
        </p>
        <p class="text-jsr-gray-700 dark:text-jsr-gray-200 text-sm">
          Before creating a new scope, please read the{" "}
          <a href="/docs/usage-policy#scope-names" class="link">
            scope naming policy
          </a>.
        </p>
      </>
    );
  }

  return (
    <>
      <select
        class="w-full mt-4 block py-2 px-4 input-container input select dark:bg-jsr-gray-900"
        onChange={(e) => scope.value = e.currentTarget.value}
        value={scope}
        disabled={locked}
        data-locked={locked || undefined}
      >
        <option value="" disabled selected class="hidden text-jsr-gray-100">
          ---
        </option>
        {scopes.value.map((scope, idx) => (
          <option key={idx} value={scope}>{scope}</option>
        ))}
      </select>

      {!locked && (
        <p class="text-tertiary">
          or{" "}
          <button
            type="button"
            class="inline link mt-2"
            onClick={() => {
              explicitCreateScope.value = true;
              scope.value = "";
            }}
          >
            create a new scope
          </button>
        </p>
      )}
    </>
  );
}

function CreateScope(
  props: {
    initialValue: string | undefined;
    onCreate: (scope: string, description: string) => void;
    locked: boolean;
    user: User;
  },
) {
  const newScope = useSignal(props.initialValue ?? "");
  const description = useSignal("");
  const errorCode = useSignal("");
  const error = useSignal("");
  const message = useComputed(() => {
    if (error.value) return error.value;
    const validationError = validateScopeName(newScope.value);
    if (validationError) {
      return validationError;
    }
    const descriptionError = validateScopeDescription(description.value);
    if (descriptionError) {
      return descriptionError;
    }
    return "";
  });

  async function onSubmit(e: Event) {
    e.preventDefault();

    const resp = await api.post<Scope>(path`/scopes`, {
      scope: newScope.value,
      description: description.value,
    });
    if (resp.ok) {
      props.onCreate(newScope.value, description.value);
    } else {
      console.error(resp);
      errorCode.value = resp.code;
      error.value = resp.message;
    }
  }

  return (
    <>
      <form class="flex flex-wrap gap-4 items-center mb-2" onSubmit={onSubmit}>
        <label class="flex items-center w-full md:w-full input-container pl-4 py-[2px] pr-[2px]">
          <span>@</span>
          <input
            class="input py-1.5 pr-4 pl-[1px] flex-grow-1 rounded-md"
            type="text"
            name="scope"
            placeholder="foo"
            disabled={props.locked}
            data-locked={props.locked || undefined}
            value={newScope}
            onInput={(e) => {
              newScope.value = e.currentTarget.value;
              error.value = "";
              errorCode.value = "";
            }}
            onBlur={(e) => {
              const newScope = e.currentTarget.value;
              if (newScope !== "" && newScope.length < 2) {
                error.value = "Scope name must be at least 2 characters long.";
              }
            }}
          />
        </label>
        <label class="flex items-center w-full md:w-full input-container pl-4 py-[2px] pr-[2px]">
          <textarea
            class="input py-1.5 pr-4 pl-[1px] flex-grow-1 rounded-md"
            name="description"
            placeholder="Enter a description for the scope (optional)"
            disabled={props.locked}
            data-locked={props.locked || undefined}
            value={description}
            onInput={(e) => {
              description.value = e.currentTarget.value;
              error.value = "";
              errorCode.value = "";
            }}
            onBlur={(e) => {
              const newDescription = e.currentTarget.value;
              const descriptionError = validateScopeDescription(newDescription);
              if (descriptionError) {
                error.value = descriptionError;
              }
            }}
          />
        </label>
        <button type="submit" class="button-primary">Create</button>
        {errorCode.value === "scopeNameReserved" && (
          <div class="mt-3 w-full space-y-4 bg-jsr-yellow-50 border-1.5 border-jsr-yellow-200 dark:border-jsr-yellow-700 dark:bg-jsr-yellow-900/30 p-4 md:p-6 rounded-xl">
            <div class="mb-2">
              The provided scope name is reserved. Please use the form below to
              claim it if you think you have a valid reason to do so.
            </div>
            <TicketModal
              user={props.user}
              kind="scope_claim"
              extraMeta={{ "scope": newScope.value }}
              title="Request reserved scope name"
              description={
                <>
                  <p class="mt-4 text-secondary">
                    The scope name '@{newScope.value}' is reserved. If you think
                    you have a valid reason to claim it, such as proof of
                    ownership of a package or scope on npm with the same name,
                    please provide the details below.
                  </p>
                </>
              }
              fields={[{
                name: "message",
                label: "Reason",
                type: "textarea",
                required: true,
              }]}
              style="primary"
            >
              Request reserved scope
            </TicketModal>
          </div>
        )}
      </form>
      {newScope.value.includes("_")
        ? (
          <p class="text-sm text-jsr-yellow-600">
            Scope names can not contain _, use - instead.{" "}
            {!props.locked && (
              <button
                type="button"
                class="text-jsr-cyan-700 hover:underline hover:text-blue-400"
                onClick={() => {
                  newScope.value = newScope.value.replace(/_/g, "-");
                }}
              >
                Click to replace
              </button>
            )}
          </p>
        )
        : message.value && errorCode.value !== "scopeNameReserved" && (
          <p class="text-sm text-jsr-yellow-600">{message}</p>
        )}
    </>
  );
}

export function PackageName(
  { scope, name, pkg, locked }: {
    scope: Signal<string>;
    name: Signal<string>;
    pkg: Signal<Package | null | undefined>;
    locked: boolean;
  },
) {
  const error = useSignal("");
  const message = useComputed(() => {
    if (error.value) return error.value;
    if (name.value.length === 0) return "";
    const validationError = validatePackageName(name.value);
    if (validationError) {
      return validationError;
    }
    return "";
  });

  useSignalEffect(() => {
    const scope_ = scope.value;
    const newName = name.value;
    if (
      scope_ === "" || newName.length < 2 ||
      !/^[a-z0-9][a-z0-9\-]+$/.test(scope_) ||
      !/^[a-z0-9][a-z0-9\-]+$/.test(newName)
    ) {
      pkg.value = undefined;
      return;
    }
    pkg.value = undefined;
    const controller = new AbortController();
    new Promise((resolve) => setTimeout(resolve, 200)).then(async () => {
      if (controller.signal.aborted) return;
      await api.get<Package>(
        path`/scopes/${scope_}/packages/${newName}`,
        undefined,
        { signal: controller.signal },
      ).then((resp) => {
        if (scope_ === scope.value && newName === name.value) {
          if (resp.ok) {
            pkg.value = resp.data;
          } else if (
            resp.code === "packageNotFound" || resp.code === "malformedRequest"
          ) {
            pkg.value = null;
          } else {
            console.error(resp);
            error.value = resp.message;
            pkg.value = null;
          }
        }
      });
    });
    return () => controller.abort();
  });

  return (
    <>
      <input
        class="w-full md:w-full block h-10 px-4 input-container input"
        type="text"
        name="package"
        placeholder="bar"
        disabled={locked}
        data-locked={locked || undefined}
        value={name}
        onInput={(e) => {
          name.value = e.currentTarget.value;
          pkg.value = undefined;
          error.value = "";
        }}
        onBlur={(e) => {
          const newPackage = e.currentTarget.value;
          if (newPackage !== "" && newPackage.length < 2) {
            error.value = "Package name must be at least 2 characters long.";
          }
        }}
      />
      {name.value.includes("_")
        ? (
          <p class="text-sm text-jsr-yellow-600">
            Package names can not contain _, use - instead. {!locked && (
              <button
                type="button"
                class="text-jsr-cyan-700 hover:underline hover:text-blue-400"
                onClick={() => {
                  name.value = name.value.replace(/_/g, "-");
                }}
              >
                Click to replace
              </button>
            )}
          </p>
        )
        : message.value && <p class="text-sm text-jsr-yellow-600">{message}</p>}
    </>
  );
}

export function CreatePackage({ scope, name, pkg, fromCli }: {
  scope: Signal<string>;
  name: Signal<string>;
  pkg: Signal<Package | null | undefined>;
  fromCli: boolean;
}) {
  const error = useSignal("");
  useSignalEffect(() => {
    scope.value;
    name.value;
    error.value = "";
  });

  if (
    scope.value.length === 0 || name.value.length < 2 || pkg.value === undefined
  ) return null;

  return (
    <div class="max-w-2xl mt-12 bg-jsr-cyan-50 dark:bg-jsr-cyan-950 border-1.5 border-jsr-cyan-200 dark:border-jsr-cyan-700 rounded-lg p-4 md:p-6 overflow-x-hidden flex flex-wrap sm:flex-nowrap! justify-between items-center gap-8">
      {pkg.value === null
        ? (
          <>
            <div>
              <p class="dark:text-white">
                The package{" "}
                <code class="text-jsr-cyan-800 dark:text-jsr-cyan-200">
                  @{scope}/{name}
                </code>{" "}
                does not exist yet. Create it now to publish your package.
              </p>
              {error && <p class="text-sm text-jsr-yellow-600">{error}</p>}
            </div>
            <div>
              <button
                type="button"
                class="button-primary"
                onClick={async () => {
                  error.value = "";
                  const resp = await api.post<Package>(
                    path`/scopes/${scope.value}/packages`,
                    { package: name.value },
                  );
                  if (resp.ok) {
                    if (fromCli) {
                      pkg.value = resp.data;
                    } else {
                      location.href = `/@${scope.value}/${name.value}/publish`;
                    }
                  } else {
                    error.value = resp.message;
                    console.error(resp);
                  }
                }}
              >
                Create
              </button>
            </div>
          </>
        )
        : (
          <>
            <div>
              <p class="font-bold text-lg leading-none">
                <a
                  href={`/@${pkg.value.scope}/${pkg.value.name}`}
                  class="hover:underline"
                >
                  @{pkg.value.scope}/{pkg.value.name}
                </a>
              </p>
              <p>{pkg.value.description || <i>No description</i>}</p>
              <p class="text-tertiary">
                Created {twas(new Date(pkg.value.createdAt).getTime())}.
              </p>
              {fromCli && (
                <p class="mt-2 text-tertiary">
                  You can now close this page and go back to your terminal to
                  continue publishing.
                </p>
              )}
            </div>
            {!fromCli && (
              <div>
                <a
                  class="button-primary"
                  href={`/@${pkg.value.scope}/${pkg.value.name}/publish`}
                >
                  Publish
                </a>
              </div>
            )}
          </>
        )}
    </div>
  );
}
