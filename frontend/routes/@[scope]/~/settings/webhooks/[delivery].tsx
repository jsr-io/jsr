// Copyright 2024 the JSR authors. All rights reserved. MIT license.
import { HttpError, RouteConfig } from "fresh";
import { define } from "../../../../../util.ts";
import { ScopeHeader } from "../../../(_components)/ScopeHeader.tsx";
import { ScopeNav } from "../../../(_components)/ScopeNav.tsx";
import { WebhookEdit } from "../../../../../islands/WebhookEdit.tsx";
import type {
  FullScope,
  WebhookDelivery as ApiWebhookDelivery,
  WebhookEndpoint,
} from "../../../../../utils/api_types.ts";
import { scopeDataWithMember } from "../../../../../utils/data.ts";
import { path } from "../../../../../utils/api.ts";
import { scopeIAM } from "../../../../../utils/iam.ts";
import { WebhookDelivery } from "../../../../../components/WebhookDelivery.tsx";

export default define.page<typeof handler>(function ScopeSettingsPage(
  { data },
) {
  return (
    <div class="mb-20">
      <ScopeHeader scope={data.scope} />
      <ScopeNav active="Settings" iam={data.iam} scope={data.scope.scope} />
      <WebhookDelivery webhook={data.webhook} delivery={data.webhookDelivery} />
    </div>
  );
});

export const handler = define.handlers({
  async GET(ctx) {
    const [user, data, webhookResp, webhookDeliveryResp] = await Promise.all([
      ctx.state.userPromise,
      scopeDataWithMember(ctx.state, ctx.params.scope),
      ctx.state.api.get<WebhookEndpoint>(
        path`/scopes/${ctx.params.scope}/webhooks/${ctx.params.webhook}`,
      ),
      ctx.state.api.get<ApiWebhookDelivery>(
        path`/scopes/${ctx.params.scope}/webhooks/${ctx.params.webhook}/deliveries/${ctx.params.delivery}`,
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
    if (!webhookDeliveryResp.ok) {
      if (webhookDeliveryResp.code === "webhookNotFound") {
        throw new HttpError(404, "The webhook was not found.");
      }
      throw webhookDeliveryResp; // graceful handle errors
    }

    ctx.state.meta = { title: `Webhook Settings - @${data.scope.scope} - JSR` };
    return {
      data: {
        scope: data.scope as FullScope,
        webhook: webhookResp.data,
        webhookDelivery: webhookDeliveryResp.data,
        iam,
      },
    };
  },
});

export const config: RouteConfig = {
  routeOverride: "/@:scope/~/settings/webhooks/:webhook/deliveries/:delivery",
};
