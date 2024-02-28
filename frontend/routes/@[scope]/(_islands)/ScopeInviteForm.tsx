// Copyright 2024 the JSR authors. All rights reserved. MIT license.
import { useSignal } from "@preact/signals";
import { useCallback } from "preact/hooks";
import { JSX } from "preact/jsx-runtime";
import { ScopeInvite } from "../../../utils/api_types.ts";
import { api, path } from "../../../utils/api.ts";

interface ScopeInviteFormProps {
  scope: string;
}

export function ScopeInviteForm(props: ScopeInviteFormProps) {
  const submitting = useSignal(false);
  const error = useSignal<string>("");

  const onSubmit = useCallback(
    (e: JSX.TargetedEvent<HTMLFormElement>) => {
      e.preventDefault();
      const formData = new FormData(e.currentTarget);
      const githubLogin = String(formData.get("githubLogin"));

      submitting.value = true;

      api.post<ScopeInvite>(
        path`/scopes/${props.scope}/members`,
        { githubLogin },
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
        <input
          class="block w-full max-w-sm p-1.5 input-container input"
          type="text"
          name="githubLogin"
          placeholder="GitHub username"
          required
          disabled={submitting}
        />
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
