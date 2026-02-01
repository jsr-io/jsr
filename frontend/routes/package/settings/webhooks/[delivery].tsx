// Copyright 2024 the JSR authors. All rights reserved. MIT license.
import { HttpError, RouteConfig } from "fresh";
import { define } from "../../../../util.ts";
import { PackageHeader } from "../../(_components)/PackageHeader.tsx";
import { PackageNav, Params } from "../../(_components)/PackageNav.tsx";
import type {
  WebhookDelivery as ApiWebhookDelivery,
  WebhookEndpoint,
} from "../../../../utils/api_types.ts";
import { packageData } from "../../../../utils/data.ts";
import { path } from "../../../../utils/api.ts";
import { scopeIAM } from "../../../../utils/iam.ts";
import { WebhookDelivery } from "../../../../components/WebhookDelivery.tsx";

export default define.page<typeof handler>(function ScopeSettingsPage(
  { data, params },
) {
  return (
    <div class="mb-20">
      <PackageHeader
        package={data.package}
        downloads={data.downloads}
      />

      <PackageNav
        currentTab="Settings"
        versionCount={data.package.versionCount}
        dependencyCount={data.package.dependencyCount}
        dependentCount={data.package.dependentCount}
        iam={data.iam}
        params={params as unknown as Params}
        latestVersion={data.package.latestVersion}
      />

      <WebhookDelivery delivery={data.webhookDelivery} />
    </div>
  );
});

export const handler = define.handlers({
  async GET(ctx) {
    const [user, data, webhookResp, webhookDeliveryResp] = await Promise.all([
      ctx.state.userPromise,
      packageData(ctx.state, ctx.params.scope, ctx.params.package),
      ctx.state.api.get<WebhookEndpoint>(
        path`/scopes/${ctx.params.scope}/packages/${ctx.params.package}/webhooks/${ctx.params.webhook}`,
      ),
      ctx.state.api.get<ApiWebhookDelivery>(
        path`/scopes/${ctx.params.scope}/packages/${ctx.params.package}/webhooks/${ctx.params.webhook}/deliveries/${ctx.params.delivery}`,
      ),
    ]);
    if (user instanceof Response) return user;
    if (data === null) throw new HttpError(404, "The scope was not found.");

    const { pkg, scopeMember, downloads } = data;
    const iam = scopeIAM(ctx.state, scopeMember, user);
    if (!iam.canAdmin) throw new HttpError(404, "This package was not found.");

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

    ctx.state.meta = {
      title: `Webhook Settings - @${pkg.scope}/${pkg.name} - JSR`,
    };
    return {
      data: {
        package: pkg,
        downloads,
        webhook: webhookResp.data,
        webhookDelivery: webhookDeliveryResp.data,
        iam,
      },
    };
  },
});

export const config: RouteConfig = {
  routeOverride: "/@:scope/:package/settings/webhooks/:webhook/deliveries/:delivery",
};
