// Copyright 2024 the JSR authors. All rights reserved. MIT license.
import { useSignal } from "@preact/signals";
import { useCallback } from "preact/hooks";
import { api, path } from "../../../utils/api.ts";
import { IS_BROWSER } from "$fresh/runtime.ts";

export interface ScopeMemberRoleProps {
  scope: string;
  userId: string;
  isAdmin: boolean;
  isLastAdmin: boolean;
}

export function ScopeMemberRole(props: ScopeMemberRoleProps) {
  const role = useSignal(props.isAdmin ? "admin" : "member");
  const selected = useSignal(role.peek());
  const submitting = useSignal(false);

  const onSave = useCallback(() => {
    submitting.value = true;
    api.patch(
      path`/scopes/${props.scope}/members/${props.userId}`,
      { isAdmin: selected.value === "admin" },
    ).then((res) => {
      submitting.value = false;
      if (!res.ok) {
        console.error(res);
        alert(res.message);
        return;
      }
      role.value = selected.value;
      location.reload();
    }).catch((err) => {
      console.error(err);
      alert("An unknown error occurred");
    });
  }, []);

  return (
    <div class="flex flex-col items-start">
      <select
        class={"block w-32 px-3 py-1.5 input-container select" +
          (props.isLastAdmin ? " cursor-not-allowed" : " disabled:cursor-wait")}
        value={selected}
        onInput={(e) => selected.value = e.currentTarget.value}
        disabled={!IS_BROWSER || props.isLastAdmin || submitting}
        title={props.isLastAdmin
          ? "This is the last admin in this scope. Promote another member to admin before demoting this one."
          : undefined}
      >
        <option value="member">Member</option>
        <option value="admin">Admin</option>
      </select>
      {role.value !== selected.value && (
        <button
          class="mt-2 button-primary"
          disabled={submitting}
          onClick={onSave}
        >
          Save
        </button>
      )}
    </div>
  );
}
