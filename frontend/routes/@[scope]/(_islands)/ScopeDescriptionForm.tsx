import { useSignal } from "@preact/signals";
import { useState } from "preact/hooks";
import { TbPencil, TbCheck, TbX } from "tb-icons";
import { api, path } from "../../../utils/api.ts";
import type { FullScope } from "../../../utils/api_types.ts";

interface ScopeDescriptionFormProps {
  scope: FullScope;
}

export function ScopeDescriptionForm(
  { scope: initialScope }: ScopeDescriptionFormProps,
) {
  const scope = useSignal(initialScope);
  const isEditing = useSignal(false);
  const editedDescription = useSignal(scope.value.description ?? "");
  const [error, setError] = useState<string | null>(null);
  const [isLoading, setIsLoading] = useState(false);

  async function handleSave() {
    setIsLoading(true);
    setError(null);
    const resp = await api.patch(path`/scopes/${scope.value.scope}`, {
      description: editedDescription.value,
    });

    setIsLoading(false);
    if (resp.ok) {
      // Update the local scope signal with the new description
      scope.value = { ...scope.value, description: editedDescription.value };
      isEditing.value = false;
    } else {
      setError(resp.message ?? "Failed to update description.");
      console.error("Failed to save scope description:", resp);
    }
  }

  function handleCancel() {
    editedDescription.value = scope.value.description ?? "";
    isEditing.value = false;
    setError(null);
  }

  if (isEditing.value) {
    return (
      <div class="mt-2 space-y-2">
        <textarea
          class="w-full p-2 border rounded input"
          value={editedDescription}
          onInput={(e) => editedDescription.value = e.currentTarget.value}
          placeholder="Enter a description for the scope"
          rows={3}
          disabled={isLoading}
        />
        {error && <p class="text-sm text-red-600">{error}</p>}
        <div class="flex gap-2">
          <button
            type="button"
            class="button-primary"
            onClick={handleSave}
            disabled={isLoading}
          >
            <TbCheck class="size-5" />
            {isLoading ? "Saving..." : "Save"}
          </button>
          <button
            type="button"
            class="button-danger"
            onClick={handleCancel}
            disabled={isLoading}
          >
            <TbX class="size-5" />
            Cancel
          </button>
        </div>
      </div>
    );
  }

  return (
    <div class="mt-2 flex items-start gap-2">
      <p class="text-secondary max-w-2xl flex-grow">
        {scope.value.description || <i>No description provided.</i>}
      </p>
        <button
          type="button"
          class="button-primary flex-shrink-0"
          onClick={() => isEditing.value = true}
          aria-label="Edit description"
        >
          <TbPencil class="size-5" />
          Edit
        </button>
    </div>
  );
}
