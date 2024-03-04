// Copyright 2024 the JSR authors. All rights reserved. MIT license.
import { useSignal } from "@preact/signals";
import { useCallback, useRef } from "preact/hooks";
import { JSX } from "preact/jsx-runtime";
import { ScopeInvite } from "../../../utils/api_types.ts";
import { api, path } from "../../../utils/api.ts";

interface ScopeInviteFormProps {
  scope: string;
}

export function ScopeInviteForm(props: ScopeInviteFormProps) {
  const submitting = useSignal(false);
  const error = useSignal<string>("");
  const kind = useSignal<"github" | "uuid">("github");
  const inputRef = useRef<HTMLInputElement>(null);

  const onSubmit = useCallback(
    (e: JSX.TargetedEvent<HTMLFormElement>) => {
      e.preventDefault();
      const formData = new FormData(e.currentTarget);

      const kind = String(formData.get("kind"));
      const inviteValue = String(formData.get("inviteValue"));

      submitting.value = true;

      api.post<ScopeInvite>(
        path`/scopes/${props.scope}/members`,
        {
          githubLogin: kind === "github" ? inviteValue : undefined,
          uuid: kind === "uuid" ? inviteValue : undefined,
        },
      ).then((res) => {
        submitting.value = false;
        if (!res.ok) {
          error.value = res.message;
          return;
        }
        error.value = "";
        location.reload();
      }).catch((err) => {
        console.error(err);
        error.value = "An unknown error occurred";
      });
    },
    [],
  );

  return (
    <form
      method="POST"
      class="contents"
      onSubmit={onSubmit}
    >
      <div class="mt-4 flex gap-4">
        <div class="flex">
          <select
            name="kind"
            id="kind-select"
            class="inline-block p-1.5 input-container input rounded-r-none border-r-0"
            disabled={submitting}
            onChange={(e) => {
              if (kind.value !== e.currentTarget.value) {
                kind.value = e.currentTarget.value as "github" | "uuid";
                if (inputRef?.current) {
                  inputRef.current.value = "";
                }
              }
            }}
          >
            <option value="github" selected>GitHub</option>
            <option value="uuid">User ID</option>
          </select>
          <input
            class="inline-block w-full max-w-sm px-3 input-container text-sm input rounded-l-none"
            type="text"
            name="inviteValue"
            placeholder={kind.value === "github"
              ? "GitHub username"
              : "User ID"}
            required
            ref={inputRef}
            disabled={submitting}
          />
        </div>
        <button
          class="button-primary"
          type="submit"
          name="action"
          value="invite"
          disabled={submitting}
        >
          Invite
        </button>
      </div>
      {error && <p class="text-red-600 mt-2">{error}</p>}
    </form>
  );
}
