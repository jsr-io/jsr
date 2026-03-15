// Copyright 2024 the JSR authors. All rights reserved. MIT license.
import { useEffect, useId, useRef } from "preact/hooks";
import { useSignal } from "@preact/signals";
import { TbLoader2 } from "tb-icons";
import { api, path } from "../../../utils/api.ts";
import type { Scope, ScopeMember } from "../../../utils/api_types.ts";

interface ScopeInfo {
  scope: string;
  memberCount: number;
}

export function DeleteUser(
  props: { userId: string; userName: string },
) {
  const open = useSignal(false);
  const status = useSignal<"pending" | "loading" | "submitting">("pending");
  const scopes = useSignal<ScopeInfo[]>([]);
  const ref = useRef<HTMLDivElement>(null);
  const buttonRef = useRef<HTMLButtonElement>(null);
  const prefix = useId();

  useEffect(() => {
    function outsideClick(e: Event) {
      if (
        ref.current && !ref.current.contains(e.target as Element) &&
        buttonRef.current && !buttonRef.current.contains(e.target as Element)
      ) {
        open.value = false;
      }
    }
    document.addEventListener("click", outsideClick);
    return () => document.removeEventListener("click", outsideClick);
  }, []);

  useEffect(() => {
    if (!open.value && status.value !== "pending") {
      setTimeout(() => {
        status.value = "pending";
      }, 200);
    }
  }, [open.value]);

  function onOpen() {
    open.value = !open.value;
    if (open.value) {
      status.value = "loading";
      api.get<Scope[]>(path`/users/${props.userId}/scopes`).then(
        async (res) => {
          if (res.ok) {
            const memberResponses = await Promise.all(
              res.data.map((s) =>
                api.get<ScopeMember[]>(path`/scopes/${s.scope}/members`)
              ),
            );
            scopes.value = res.data.map((s, i) => ({
              scope: s.scope,
              memberCount: memberResponses[i].ok
                ? memberResponses[i].data.length
                : 0,
            }));
          }
          status.value = "pending";
        },
      );
    }
  }

  function onConfirm() {
    status.value = "submitting";
    api.delete(path`/admin/users/${props.userId}`).then((res) => {
      if (res.ok) {
        location.reload();
      } else {
        console.error(res);
        status.value = "pending";
        alert("Failed to delete user");
      }
    });
  }

  return (
    <>
      <button
        ref={buttonRef}
        id={`${prefix}-delete-modal`}
        type="button"
        class="button-danger"
        onClick={onOpen}
        aria-expanded={open.value ? "true" : "false"}
      >
        delete
      </button>
      <div
        class={`fixed top-0 right-0 w-screen h-screen bg-gray-300/40 dark:bg-jsr-gray-950/70 z-80 flex justify-center items-center overflow-hidden ${
          open.value ? "opacity-100" : "opacity-0 pointer-events-none"
        } transition`}
        aria-labelledby={`${prefix}-delete-modal`}
        role="region"
        style="--tw-shadow-color: rgba(156,163,175,0.2);"
      >
        <div
          ref={ref}
          class={`z-90 rounded border-1.5 border-current dark:border-cyan-700 bg-white dark:bg-jsr-gray-950 shadow min-w-96 max-w-[95vw] max-h-[95vh] px-6 py-5 ${
            open.value ? "translate-y-0" : "translate-y-5"
          } transition`}
          style="--tw-shadow-color: rgba(156,163,175,0.2);"
        >
          {status.value === "pending"
            ? (
              <>
                <h2 class="text-xl font-bold">
                  Delete user '{props.userName}'
                </h2>
                <p class="mt-3 text-secondary max-w-lg">
                  Are you sure you want to delete this user? This action cannot
                  be undone.
                </p>

                {scopes.value.length > 0 && (
                  <div class="mt-4">
                    <p class="text-sm font-medium mb-2">
                      The following scopes will be affected:
                    </p>
                    <ul class="text-sm border rounded divide-y max-h-48 overflow-y-auto">
                      {scopes.value.map((scope) => (
                        <li
                          key={scope.scope}
                          class="px-3 py-2 flex items-center justify-between gap-4"
                        >
                          <a
                            href={`/@${scope.scope}`}
                            target="_blank"
                            class="link"
                          >
                            @{scope.scope}
                          </a>
                          <span class="text-secondary text-xs">
                            {scope.memberCount <= 1
                              ? "transferred to service account"
                              : "transferred to another user in scope"}
                          </span>
                        </li>
                      ))}
                    </ul>
                  </div>
                )}

                <div class="flex justify-end gap-3 mt-5">
                  <button
                    type="button"
                    class="button-primary"
                    onClick={() => open.value = false}
                  >
                    Cancel
                  </button>
                  <button
                    type="button"
                    class="button-danger"
                    onClick={onConfirm}
                  >
                    Delete user
                  </button>
                </div>
              </>
            )
            : (
              <div class="flex flex-col gap-3 items-center justify-center py-6">
                <TbLoader2 class="w-8 h-8 animate-spin" />
                <p class="text-secondary">
                  {status.value === "loading"
                    ? "Loading scope info..."
                    : "Deleting user..."}
                </p>
              </div>
            )}
        </div>
      </div>
    </>
  );
}
