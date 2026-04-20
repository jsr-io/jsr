// Copyright 2024 the JSR authors. All rights reserved. MIT license.
import { useSignal } from "@preact/signals";
import { api, path } from "../utils/api.ts";

export function WebhookRedeliverButton(
  { scope, package: pkg, webhookId, deliveryId }: {
    scope: string;
    package?: string;
    webhookId: string;
    deliveryId: string;
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
            ? path`/scopes/${scope}/packages/${pkg}/webhooks/${webhookId}/deliveries/${deliveryId}/redeliver`
            : path`/scopes/${scope}/webhooks/${webhookId}/deliveries/${deliveryId}/redeliver`,
          {},
        ).then((res) => {
          processing.value = false;
          if (res.ok) {
            location.reload();
          } else {
            alert(`Redeliver failed: ${res.message}`);
          }
        });
      }}
    >
      {processing.value ? "Redelivering..." : "Redeliver"}
    </button>
  );
}
