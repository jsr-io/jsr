// Copyright 2024 the JSR authors. All rights reserved. MIT license.
import { HttpError, RouteConfig } from "fresh";
import { define } from "../../../../util.ts";
import { WebhookEdit } from "../../../../islands/WebhookEdit.tsx";
import {
  WebhookDelivery,
  WebhookEndpoint,
} from "../../../../utils/api_types.ts";
import { packageData } from "../../../../utils/data.ts";
import { path } from "../../../../utils/api.ts";
import { scopeIAM } from "../../../../utils/iam.ts";
import { PackageHeader } from "../../(_components)/PackageHeader.tsx";
import { PackageNav, Params } from "../../(_components)/PackageNav.tsx";
import {
  WebhookDeliveries,
} from "../../../../components/WebhookDeliveries.tsx";
import { WebhookTestButton } from "../../../../islands/WebhookTestButton.tsx";

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

      <WebhookEdit
        webhook={data.webhook}
        scope={data.package.scope}
        package={data.package.name}
      />

      <div class="border-t pt-8 mt-12">
        <div class="flex items-center justify-between">
          <h2 class="text-lg sm:text-xl font-semibold">Test</h2>
          <WebhookTestButton
            scope={data.package.scope}
            package={data.package.name}
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
      packageData(ctx.state, ctx.params.scope, ctx.params.package),
      ctx.state.api.get<WebhookEndpoint>(
        path`/scopes/${ctx.params.scope}/packages/${ctx.params.package}/webhooks/${ctx.params.webhook}`,
      ),
      ctx.state.api.get<WebhookDelivery[]>(
        path`/scopes/${ctx.params.scope}/webhooks/${ctx.params.webhook}/deliveries`,
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
    if (!webhookDeliveriesResp.ok) {
      if (webhookDeliveriesResp.code === "webhookNotFound") {
        throw new HttpError(404, "The webhook was not found.");
      }
      throw webhookDeliveriesResp; // graceful handle errors
    }

    ctx.state.meta = {
      title: `Webhook Settings - @${pkg.scope}/${pkg.name} - JSR`,
      description: `@${pkg.scope}/${pkg.name} on JSR${
        pkg.description ? `: ${pkg.description}` : ""
      }`,
    };

    return {
      data: {
        package: pkg,
        downloads,
        iam,
        webhook: webhookResp.data,
        webhookDeliveries: webhookDeliveriesResp.data,
      },
    };
  },
});

export const config: RouteConfig = {
  routeOverride: "/@:scope/:package/settings/webhooks/:webhook",
};
