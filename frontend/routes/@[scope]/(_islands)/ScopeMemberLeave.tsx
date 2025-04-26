// Copyright 2024 the JSR authors. All rights reserved. MIT license.
import { useSignal } from "@preact/signals";
import { useEffect } from "preact/hooks";
import { TbArrowRightFromArc } from "tb-icons";

export function ScopeMemberLeave({
  userId,
  isAdmin,
  isLastAdmin,
  scopeName = "",
}: {
  userId: string;
  isAdmin: boolean;
  isLastAdmin: boolean;
  scopeName?: string;
}) {
  const scopeInput = useSignal("");
  const isEmptyInput = useSignal(false);
  const isInvalidInput = useSignal(false);

  useEffect(() => {
    const handler = setTimeout(() => {
      validate();
    }, 300);

    return () => clearTimeout(handler);
  }, [scopeInput.value]);

  const validate = () => {
    isEmptyInput.value = scopeInput.value.length === 0;
    isInvalidInput.value = scopeInput.value !== scopeName &&
      scopeInput.value.length > 0;
  };

  return (
    <form
      method="POST"
      class="max-w-3xl border-t border-jsr-cyan-950/10 pt-8 mt-12"
    >
      <h2 class="text-lg font-semibold">Leave scope</h2>
      <p class="mt-2 text-jsr-gray-600">
        Leaving this scope will revoke your access to all packages in this
        scope. You will no longer be able to publish packages to this
        scope{isAdmin && " or manage members"}.
      </p>
      <input type="hidden" name="userId" value={userId} />
      <div class="mt-4 flex justify-between gap-4">
        <input
          type="text"
          class="inline-block w-full max-w-sm px-3 input-container text-sm input"
          value={scopeInput.value}
          onInput={(e) => {
            scopeInput.value = (e.target as HTMLInputElement).value;
          }}
          placeholder="Scope name"
          disabled={isLastAdmin}
          title={isLastAdmin
            ? "This is the last admin in this scope. Promote another member to admin before demoting this one."
            : undefined}
        />
        <button
          class="button-danger"
          type="submit"
          name="action"
          value="deleteMember"
          disabled={isLastAdmin || isInvalidInput.value || isEmptyInput.value}
        >
          Leave
          <TbArrowRightFromArc class="size-5 ml-2 rotate-180" />
        </button>
      </div>
      {(isLastAdmin || isInvalidInput.value) && (
        <div class="mt-6 border rounded-md border-red-300 bg-red-50 p-6 text-red-600 dark:bg-red-900/10 dark:text-red-400">
          <span class="font-bold text-xl">Warning</span>
          <p>
            {isLastAdmin &&
              "You are the last admin in this scope. You must promote another member to admin before leaving."}
            {isInvalidInput.value &&
              "The scope name you entered does not match the scope name."}
          </p>
        </div>
      )}
    </form>
  );
}
