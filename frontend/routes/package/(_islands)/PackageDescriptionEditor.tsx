// Copyright 2024 the JSR authors. All rights reserved. MIT license.
import { useSignal } from "@preact/signals";

export function PackageDescriptionEditor(props: { description: string }) {
  const description = useSignal<string>(props.description);

  return (
    <div class="flex flex-col items-start gap-4">
      <textarea
        class="w-full min-w-80 max-w-2xl block px-2 py-2 text-sm input-container input"
        name="description"
        placeholder=""
        rows={2}
        onInput={(e) => description.value = e.currentTarget.value}
      >
        {description}
      </textarea>

      <button
        class="button-primary"
        type="submit"
        name="action"
        value="updateDescription"
        disabled={description.value === props.description}
      >
        Save
      </button>
    </div>
  );
}
