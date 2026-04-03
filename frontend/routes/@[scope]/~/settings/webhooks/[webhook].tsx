// Copyright 2024 the JSR authors. All rights reserved. MIT license.
import { HttpError } from "fresh";
import { define } from "../../../../../util.ts";
import { ScopeHeader } from "../../../(_components)/ScopeHeader.tsx";
import { ScopeNav } from "../../../(_components)/ScopeNav.tsx";
import { WebhookEdit } from "../../../../../islands/WebhookEdit.tsx";
import {
  FullScope,
  WebhookDelivery,
  WebhookEndpoint,
} from "../../../../../utils/api_types.ts";
import { scopeDataWithMember } from "../../../../../utils/data.ts";
import { path } from "../../../../../utils/api.ts";
import { scopeIAM } from "../../../../../utils/iam.ts";
import {
  WebhookDeliveries,
} from "../../../../../components/WebhookDeliveries.tsx";
import { WebhookTestButton } from "../../../../../islands/WebhookTestButton.tsx";

export default define.page<typeof handler>(function ScopeSettingsPage(
  { data },
) {
  return (
    <div class="mb-20">
      <ScopeHeader scope={data.scope} />
      <ScopeNav active="Settings" iam={data.iam} scope={data.scope.scope} />
      <WebhookEdit webhook={data.webhook} scope={data.scope.scope} />

      <div class="border-t pt-8 mt-12">
        <div class="flex items-center justify-between">
          <h2 class="text-lg sm:text-xl font-semibold">Test</h2>
          <WebhookTestButton
            scope={data.scope.scope}
            webhookId={data.webhook.id}
          />
        </div>
        <p class="text-sm text-secondary mt-2">
          Send a test event to this webhook endpoint to verify it is working
          correctly.
        </p>
      </div>

      <WebhookDeliveries
        webhook={data.webhook}
        deliveries={data.webhookDeliveries}
      />
    </div>
  );
});

export const handler = define.handlers({
  async GET(ctx) {
    const [user, data, webhookResp, webhookDeliveriesResp] = await Promise.all([
      ctx.state.userPromise,
      scopeDataWithMember(ctx.state, ctx.params.scope),
      ctx.state.api.get<WebhookEndpoint>(
        path`/scopes/${ctx.params.scope}/webhooks/${ctx.params.webhook}`,
      ),
      ctx.state.api.get<WebhookDelivery[]>(
        path`/scopes/${ctx.params.scope}/webhooks/${ctx.params.webhook}/deliveries`,
      ),
    ]);
    if (user instanceof Response) return user;
    if (data === null) throw new HttpError(404, "The scope was not found.");

    const iam = scopeIAM(ctx.state, data?.scopeMember, user);
    if (!iam.canAdmin) throw new HttpError(404, "The scope was not found.");

    if (!webhookResp.ok) {
      if (webhookResp.code === "webhookNotFound") {
        throw new HttpError(404, "The webhook was not found.");
      }
      throw webhookResp; // graceful handle errors
    }
    if (!webhookDeliveriesResp.ok) {
      if (webhookDeliveriesResp.code === "webhookNotFound") {
        throw new HttpError(404, "The webhook was not found.");
      }
      throw webhookDeliveriesResp; // graceful handle errors
    }

    ctx.state.meta = { title: `Webhook Settings - @${data.scope.scope} - JSR` };
    return {
      data: {
        scope: data.scope as FullScope,
        webhook: webhookResp.data,
        webhookDeliveries: webhookDeliveriesResp.data,
        iam,
      },
    };
  },
});
