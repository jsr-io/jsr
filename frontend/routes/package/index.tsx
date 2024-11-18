// Copyright 2024 the JSR authors. All rights reserved. MIT license.
import { HttpError, RouteConfig } from "fresh";
import { define } from "../../util.ts";
import { DocsData, packageDataWithDocs } from "../../utils/data.ts";
import { PackageNav, Params } from "./(_components)/PackageNav.tsx";
import { PackageHeader } from "./(_components)/PackageHeader.tsx";
import { DocsView } from "./(_components)/Docs.tsx";
import { scopeIAM } from "../../utils/iam.ts";

const FRONTEND_ROOT = Deno.env.get("FRONTEND_ROOT") ?? "http://jsr.test";

export default define.page<typeof handler>(function PackagePage(
  { data, params, state },
) {
  const iam = scopeIAM(state, data.member);

  return (
    <div>
      <PackageHeader
        package={data.package}
        selectedVersion={data.selectedVersion ?? undefined}
      />
      <PackageNav
        currentTab="Index"
        versionCount={data.package.versionCount}
        iam={iam}
        params={params as unknown as Params}
        latestVersion={data.package.latestVersion}
      />

      {data.docs && data.selectedVersion
        ? (
          <DocsView
            docs={data.docs}
            params={params as unknown as Params}
            selectedVersion={data.selectedVersion}
            showProvenanceBadge
          />
        )
        : (
          <div class="mt-8 text-jsr-gray-500 text-center">
            This package has not published{" "}
            {data.package.versionCount > 0
              ? "a stable release"
              : "any versions"} yet.
          </div>
        )}
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
      {},
    );
    if (res === null) {
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
    } = res as DocsData;

    if (scopeMember && pkg.versionCount === 0) {
      return new Response(null, {
        status: 303,
        headers: {
          Location: `/@${pkg.scope}/${pkg.name}/publish`,
        },
      });
    }

    ctx.state.meta = {
      title: `@${pkg.scope}/${pkg.name} - JSR`,
      description: `@${pkg.scope}/${pkg.name} on JSR${
        pkg.description ? `: ${pkg.description}` : ""
      }`,
      ogImage: `${FRONTEND_ROOT}/@${pkg.scope}/${pkg.name}/og`,
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
  routeOverride: "/@:scope/:package{@:version}?",
};
