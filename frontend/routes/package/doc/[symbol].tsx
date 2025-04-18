// Copyright 2024 the JSR authors. All rights reserved. MIT license.
import { HttpError, RouteConfig } from "fresh";
import { define } from "../../../util.ts";
import { DocsData, packageDataWithDocs } from "../../../utils/data.ts";
import { PackageHeader } from "../(_components)/PackageHeader.tsx";
import { PackageNav, Params } from "../(_components)/PackageNav.tsx";
import { DocsView } from "../(_components)/Docs.tsx";
import { scopeIAM } from "../../../utils/iam.ts";

export default define.page<typeof handler>(function Symbol(
  { data, params, state },
) {
  const iam = scopeIAM(state, data.member);

  return (
    <div>
      <PackageHeader
        package={data.package}
        selectedVersion={data.selectedVersion}
      />

      <PackageNav
        currentTab="Docs"
        versionCount={data.package.versionCount}
        dependencyCount={data.package.dependencyCount}
        dependentCount={data.package.dependentCount}
        iam={iam}
        params={params as unknown as Params}
        latestVersion={data.package.latestVersion}
      />

      <DocsView
        docs={data.docs}
        params={params as unknown as Params}
        selectedVersion={data.selectedVersion}
        user={state.user}
        scope={data.package.scope}
        pkg={data.package.name}
      />
    </div>
  );
});

export const handler = define.handlers({
  async GET(ctx) {
    const res = await packageDataWithDocs(
      ctx.state,
      ctx.params.scope,
      ctx.params.package,
      ctx.params.version,
      {
        entrypoint: ctx.params.entrypoint,
        symbol: ctx.params.symbol,
      },
    );
    if (!res) {
      throw new HttpError(
        404,
        "This package, package version, entrypoint, or symbol was not found.",
      );
    }

    if (res.kind === "redirect") {
      return new Response(null, {
        status: 307,
        headers: {
          "location": res.symbol,
        },
      });
    }

    const {
      pkg,
      scopeMember,
      selectedVersion,
      docs,
      downloads,
    } = res as DocsData;
    if (selectedVersion === null) {
      return new Response(null, {
        status: 302,
        headers: {
          Location: `/@${ctx.params.scope}/${ctx.params.package}`,
        },
      });
    }

    if (!docs?.main) {
      throw new HttpError(
        404,
        "This package, package version, entrypoint, or symbol was not found.",
      );
    }

    ctx.state.meta = {
      /* TODO: print symbol kind here (function / class / etc) */
      title: `${ctx.params.symbol}${
        ctx.params.entrypoint && ` from ${ctx.params.entrypoint}`
      } - @${ctx.params.scope}/${ctx.params.package} - JSR`,
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
      },
      headers: { ...(ctx.params.version ? { "X-Robots-Tag": "noindex" } : {}) },
    };
  },
});

export const config: RouteConfig = {
  routeOverride: "/@:scope/:package{@:version}?/doc/:entrypoint*/~/:symbol+",
};
