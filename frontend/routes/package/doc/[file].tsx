// Copyright 2024 the JSR authors. All rights reserved. MIT license.
import { HttpError, RouteConfig } from "fresh";
import { define } from "../../../util.ts";
import { DocsData, packageDataWithDocs } from "../../../utils/data.ts";
import { PackageHeader } from "../(_components)/PackageHeader.tsx";
import { PackageNav, Params } from "../(_components)/PackageNav.tsx";
import { DocsView } from "../(_components)/Docs.tsx";
import { scopeIAM } from "../../../utils/iam.ts";

export default define.page<typeof handler>(function File({
  data,
  params,
  state,
}) {
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
      />
    </div>
  );
});

export const handler = define.handlers({
  async GET(ctx) {
    if (ctx.url.pathname.endsWith("/~")) {
      return new Response(null, {
        status: 302,
        headers: {
          Location: ctx.url.pathname.slice(0, -2),
        },
      });
    }

    const res = await packageDataWithDocs(
      ctx.state,
      ctx.params.scope,
      ctx.params.package,
      ctx.params.version,
      { entrypoint: ctx.params.entrypoint },
    );
    if (!res) {
      throw new HttpError(
        404,
        "This package, package version, entrypoint, or symbol was not found.",
      );
    }

    const {
      pkg,
      scopeMember,
      selectedVersion,
      docs,
    } = res as DocsData;
    if (selectedVersion === null) {
      return new Response(null, {
        status: 302,
        headers: {
          Location: `/@${ctx.params.scope}/${ctx.params.package}`,
        },
      });
    }

    if (docs === null) {
      throw new HttpError(
        404,
        "This package, package version, or entrypoint was not found.",
      );
    }

    ctx.state.meta = {
      title: `${
        ctx.params.entrypoint || "index"
      } - @${ctx.params.scope}/${ctx.params.package} - JSR`,
      description: `@${ctx.params.scope}/${ctx.params.package} on JSR${
        pkg.description ? `: ${pkg.description}` : ""
      }`,
    };

    return {
      data: {
        package: pkg,
        selectedVersion,
        docs,
        member: scopeMember,
      },
      headers: { ...(ctx.params.version ? { "X-Robots-Tag": "noindex" } : {}) },
    };
  },
});

export const config: RouteConfig = {
  routeOverride: "/@:scope/:package{@:version}?/doc/:entrypoint([^~]*){/~}?",
};
