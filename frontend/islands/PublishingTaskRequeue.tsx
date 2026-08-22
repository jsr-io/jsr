// Copyright 2024 the JSR authors. All rights reserved. MIT license.
import { api, path } from "../utils/api.ts";
import { PublishingTask } from "../utils/api_types.ts";
import { useSignal } from "@preact/signals";

export default function PublishingTaskRequeue(
  { publishingTask }: { publishingTask: PublishingTask },
) {
  const processing = useSignal(false);

  if (
    publishingTask.status === "failure" || publishingTask.status === "success"
  ) {
    return null;
  }

  return (
    <button
      type="button"
      disabled={processing}
      onClick={() => {
        processing.value = true;
        api.post(
          path`/admin/publishing_tasks/${publishingTask.id}/requeue`,
          {},
        )
          .then((res) => {
            processing.value = false;
            if (res.ok) {
              location.reload();
            } else {
              console.error(res);
            }
          });
      }}
      class="button-danger z-20"
    >
      Re-Queue
    </button>
  );
}
