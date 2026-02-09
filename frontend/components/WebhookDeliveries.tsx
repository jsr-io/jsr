// Copyright 2024 the JSR authors. All rights reserved. MIT license.
import type {
  WebhookDelivery,
  WebhookDeliveryStatus,
  WebhookEndpoint,
} from "../utils/api_types.ts";
import { ListDisplay } from "./List.tsx";
import { TbAlertCircle, TbCheck, TbClockHour3, TbRefresh } from "tb-icons";
import { WEBHOOK_EVENTS } from "../islands/WebhookEdit.tsx";

export function WebhookDeliveries(
  { webhook, deliveries }: {
    webhook: WebhookEndpoint;
    deliveries: WebhookDelivery[];
  },
) {
  return (
    <div class="border-t pt-8 mt-12">
      <h2 class="text-lg sm:text-xl font-semibold">Deliveries</h2>
      <ListDisplay>
        {deliveries.map((entry) => ({
          href: `./${webhook.id}/deliveries/${entry.id}`,
          content: (
            <div class="grow-1 min-w-0 w-full flex flex-col md:flex-row gap-2 md:gap-4 justify-between">
              {StatusToIcon(entry.status)}

              <div class="flex-1 min-w-0 mb-2 md:mb-0">
                <div class="text-jsr-cyan-700 dark:text-cyan-400 font-semibold truncate">
                  {entry.id}
                </div>
              </div>

              <div class="flex-none whitespace-nowrap">
                {WEBHOOK_EVENTS.find((event) => event.id === entry.event)!.name}
              </div>
            </div>
          ),
        }))}
      </ListDisplay>
    </div>
  );
}

export function StatusToIcon(status: WebhookDeliveryStatus) {
  switch (status) {
    case "pending":
      return <TbClockHour3 class="size-6 stroke-blue-500 stroke-2" />;
    case "success":
      return <TbCheck class="size-6 stroke-green-500 stroke-2" />;
    case "failure":
      return <TbAlertCircle class="size-6 stroke-red-500 stroke-2" />;
    case "retrying":
      return <TbRefresh class="size-6 stroke-yellow-500 stroke-2" />;
  }
}
