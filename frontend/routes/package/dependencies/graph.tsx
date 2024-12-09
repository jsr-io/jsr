// Copyright 2024 the JSR authors. All rights reserved. MIT license.
import { HttpError, type RouteConfig } from "fresh";
import { path } from "../../../utils/api.ts";
import { scopeIAM } from "../../../utils/iam.ts";
import { define } from "../../../util.ts";
import { DependencyGraph } from "../(_islands)/DependencyGraph.tsx";
import { packageDataWithVersion } from "../../../utils/data.ts";
import { PackageHeader } from "../(_components)/PackageHeader.tsx";
import { PackageNav, type Params } from "../(_components)/PackageNav.tsx";
import type { DependencyGraphItem } from "../../../utils/api_types.ts";

export default define.page<typeof handler>(
  function DepsGraph({ data, params, state }) {
    const iam = scopeIAM(state, data.member);

    return (
      <div class="mb-20">
        <PackageHeader
          package={data.package}
          selectedVersion={data.selectedVersion}
        />

        <PackageNav
          currentTab="Dependencies"
          versionCount={data.package.versionCount}
          iam={iam}
          params={params as unknown as Params}
          latestVersion={data.package.latestVersion}
        />

        <div class="space-y-3 mt-8">
          <DependencyGraph dependencies={data.deps} />
        </div>
      </div>
    );
  },
);

export const handler = define.handlers({
  async GET(ctx) {
    const res = await packageDataWithVersion(
      ctx.state,
      ctx.params.scope,
      ctx.params.package,
      ctx.params.version,
    );
    if (res === null) {
      throw new HttpError(
        404,
        "This package or this package version was not found.",
      );
    }

    const {
      pkg,
      scopeMember,
      selectedVersion,
    } = res;

    if (selectedVersion === null) {
      return new Response(null, {
        status: 302,
        headers: {
          Location: `/@${ctx.params.scope}/${ctx.params.package}`,
        },
      });
    }

    const depsResp = await ctx.state.api.get<DependencyGraphItem[]>(
      path`/scopes/${pkg.scope}/packages/${pkg.name}/versions/${selectedVersion.version}/dependencies/graph`,
    );
    if (!depsResp.ok) throw depsResp;

    ctx.state.meta = {
      title: `Dependencies Graph - @${pkg.scope}/${pkg.name} - JSR`,
      description: `@${pkg.scope}/${pkg.name} on JSR${
        pkg.description ? `: ${pkg.description}` : ""
      }`,
    };

    return {
      data: {
        package: pkg,
        deps: depsResp.data,
        selectedVersion,
        member: scopeMember,
      },
      headers: { "X-Robots-Tag": "noindex" },
    };
  },
});

export const config: RouteConfig = {
  routeOverride: "/@:scope/:package{@:version}?/dependencies/graph",
};
