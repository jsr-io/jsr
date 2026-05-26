// Copyright 2024 the JSR authors. All rights reserved. MIT license.
import { useSignal } from "@preact/signals";
import { api, path } from "../utils/api.ts";

export function WebhookTestButton(
  { scope, package: pkg, webhookId }: {
    scope: string;
    package?: string;
    webhookId: string;
  },
) {
  const processing = useSignal(false);

  return (
    <button
      type="button"
      class="button-primary"
      disabled={processing}
      onClick={() => {
        processing.value = true;
        api.post(
          pkg
            ? path`/scopes/${scope}/packages/${pkg}/webhooks/${webhookId}/test`
            : path`/scopes/${scope}/webhooks/${webhookId}/test`,
          {},
        ).then((res) => {
          processing.value = false;
          if (res.ok) {
            location.reload();
          } else {
            alert(`Test webhook failed: ${res.message}`);
          }
        });
      }}
    >
      {processing.value ? "Sending..." : "Send test event"}
    </button>
  );
}
