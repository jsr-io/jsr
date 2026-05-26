// Copyright 2024 the JSR authors. All rights reserved. MIT license.
import { HttpError, RouteConfig } from "fresh";
import { define } from "../../../../util.ts";
import { WebhookEdit } from "../../../../islands/WebhookEdit.tsx";
import { packageData } from "../../../../utils/data.ts";
import { scopeIAM } from "../../../../utils/iam.ts";
import { PackageHeader } from "../../(_components)/PackageHeader.tsx";
import { PackageNav, Params } from "../../(_components)/PackageNav.tsx";

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
        webhook={null}
        scope={data.package.scope}
        package={data.package.name}
      />
    </div>
  );
});

export const handler = define.handlers({
  async GET(ctx) {
    const [user, data] = await Promise.all([
      ctx.state.userPromise,
      packageData(ctx.state, ctx.params.scope, ctx.params.package),
    ]);
    if (user instanceof Response) return user;
    if (data === null) throw new HttpError(404, "The scope was not found.");

    const { pkg, scopeMember, downloads } = data;

    const iam = scopeIAM(ctx.state, scopeMember, user);

    if (!iam.canAdmin) throw new HttpError(404, "This package was not found.");

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
      },
    };
  },
});

export const config: RouteConfig = {
  routeOverride: "/@:scope/:package/settings/webhooks/new",
};
