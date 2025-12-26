// Copyright 2024 the JSR authors. All rights reserved. MIT license.
import type { ComponentChildren } from "preact";
import twas from "twas";
import type {
  WebhookDelivery,
  WebhookDeliveryStatus,
  WebhookEndpoint,
} from "../utils/api_types.ts";
import { TbAlertCircle, TbCheck, TbClockHour3, TbRefresh } from "tb-icons";

export function WebhookDelivery(
  { webhook, delivery }: {
    webhook: WebhookEndpoint;
    delivery: WebhookDelivery;
  },
) {
  return (
    <div class="grid grid-cols-1 md:grid-cols-2 gap-x-8 gap-y-3 mt-8">
      <div>
        <code>{delivery.id}</code>
      </div>

      <div>
        {delivery.event.replaceAll("_", " ")}
      </div>

      <div class="flex items-center gap-2">
        {StatusToIcon(delivery.status)}
        <div>
          {delivery.status}
        </div>
      </div>

      <div title={new Date(delivery.updatedAt).toISOString()}>
        {twas(new Date(delivery.updatedAt).getTime())}
      </div>

      <div class="mt-6 space-y-5">
        <h2 class="text-2xl font-semibold">Request</h2>
        {delivery.requestHeaders && (
          <div>
            <h3 class="text-xl font-semibold">Headers</h3>
            <Code>
              {Object.entries(delivery.requestHeaders)
                .map(([k, vs]) =>
                  vs.map((v) => (
                    <div>
                      <span class="font-bold">{k}:</span> {v}
                    </div>
                  ))
                )
                .flat()}
            </Code>
          </div>
        )}

        {delivery.payload && (
          <div>
            <h3 class="text-xl font-semibold">Payload</h3>
            <Code>
              {JSON.stringify(delivery.payload, null, 2)}
            </Code>
          </div>
        )}
      </div>

      <div class="mt-6 space-y-5">
        <div class="flex items-center justify-between gap-6">
          <h2 class="text-2xl font-semibold">Response</h2>
          <div>
            {delivery.responseHttpCode && (
              <div class="flex items-center gap-2">
                <h3 class="text-xl font-semibold">HTTP Status:</h3>
                <code class={(delivery.responseHttpCode >= 200 && delivery.responseHttpCode <= 299) ? "text-green-500" : "text-red-500"}>{delivery.responseHttpCode}</code>
              </div>
            )}
          </div>
        </div>

        {delivery.responseHeaders && (
          <div>
            <h3 class="text-xl font-semibold">Headers</h3>
            <Code>
              {Object.entries(delivery.responseHeaders)
                .map(([k, vs]) =>
                  vs.map((v) => (
                    <div>
                      <span class="font-bold">{k}:</span> {v}
                    </div>
                  ))
                )
                .flat()}
            </Code>
          </div>
        )}

        {delivery.responseBody && (
          <div>
            <h3 class="text-xl font-semibold">Body</h3>
            <Code>
              {delivery.responseBody}
            </Code>
          </div>
        )}
      </div>
    </div>
  );
}

function Code({ children }: { children: ComponentChildren }) {
  return (
    <pre class="bg-slate-900 dark:bg-slate-800 text-white rounded-lg p-4 my-2 w-full max-w-full overflow-auto text-base">
      <code>
        {children}
      </code>
    </pre>
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
