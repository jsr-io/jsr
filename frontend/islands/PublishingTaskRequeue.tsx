// Copyright 2024 the JSR authors. All rights reserved. MIT license.
import { api, path } from "../utils/api.ts";
import { useState } from "preact/hooks";
import { PublishingTask } from "../utils/api_types.ts";

export default function PublishingTaskRequeue(
  { publishingTask }: { publishingTask: PublishingTask },
) {
  const [processing, setProcessing] = useState(false);

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
        setProcessing(true);
        api.post(
          path`/admin/publishing_tasks/${publishingTask.id}/requeue`,
          {},
        )
          .then((res) => {
            setProcessing(false);
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
