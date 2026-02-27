// Copyright 2024 the JSR authors. All rights reserved. MIT license.
import { HttpError, RouteConfig } from "fresh";
import { define } from "../../../util.ts";
import { DiffData, packageDataWithDiff } from "../../../utils/data.ts";
import { PackageHeader } from "../(_components)/PackageHeader.tsx";
import { PackageNav, Params } from "../(_components)/PackageNav.tsx";
import { DiffView } from "../(_components)/Docs.tsx";
import { scopeIAM } from "../../../utils/iam.ts";

export default define.page<typeof handler>(function AllSymbols(
  { data, params, state, url },
) {
  const iam = scopeIAM(state, data.member);

  return (
    <div class="mb-20">
      <PackageHeader
        package={data.package}
        selectedVersion={data.selectedVersion ?? undefined}
        downloads={data.downloads}
      />

      <PackageNav
        currentTab="Diff"
        versionCount={data.package.versionCount}
        dependencyCount={data.package.dependencyCount}
        dependentCount={data.package.dependentCount}
        iam={iam}
        params={params as unknown as Params}
        latestVersion={data.package.latestVersion}
      />

      <DiffView
        docs={data.docs}
        scope={data.package.scope}
        pkg={data.package.name}
        versions={data.versions}
        oldVersion={params.oldVersion}
        newVersion={params.newVersion}
        url={url}
        request={data.docsReq}
      />
    </div>
  );
});

export const handler = define.handlers({
  async GET(ctx) {
    const docsReq = { all_symbols: "true" } as const;
    const res = await packageDataWithDiff(
      ctx.state,
      ctx.params.scope,
      ctx.params.package,
      ctx.params.oldVersion,
      ctx.params.newVersion,
      ctx.url.searchParams.get("full"),
      docsReq,
    );
    if (!res) {
      throw new HttpError(
        404,
        "This package or this package version was not found.",
      );
    }
    if (res instanceof Response) {
      return res;
    }

    const {
      pkg,
      scopeMember,
      selectedVersion,
      docs,
      downloads,
      versions,
    } = res as DiffData;
    if (selectedVersion !== null && docs === null) {
      return new Response(null, {
        status: 302,
        headers: {
          Location: `/@${ctx.params.scope}/${ctx.params.package}/diff/${ctx
            .params.oldVersion!}...${ctx.params.newVersion!}`,
        },
      });
    }

    ctx.state.meta = {
      title: `Diff${
        ctx.params.oldVersion && ctx.params.newVersion
          ? ` ${ctx.params.oldVersion} -> ${ctx.params.newVersion}`
          : ""
      } - All symbols - @${pkg.scope}/${pkg.name} - JSR`,
      description: `@${ctx.params.scope}/${ctx.params.package} on JSR${
        pkg.description ? `: ${pkg.description}` : ""
      }`,
    };
    return {
      data: {
        package: pkg,
        downloads,
        selectedVersion,
        docs,
        member: scopeMember,
        versions,
        docsReq,
      },
      headers: { ...(ctx.params.version ? { "X-Robots-Tag": "noindex" } : {}) },
    };
  },
});

export const config: RouteConfig = {
  routeOverride:
    "/@:scope/:package/diff/{:oldVersion}?...{:newVersion}?/all_symbols",
};
