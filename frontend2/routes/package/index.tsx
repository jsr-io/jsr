// Copyright 2024 the JSR authors. All rights reserved. MIT license.
import { Handlers, PageProps, RouteConfig } from "$fresh";
import type { Package, PackageVersionWithUser } from "../../utils/api_types.ts";
import { Docs, State } from "../../util.ts";
import { ScopeMember } from "../../utils/api_types.ts";
import { DocsData, packageDataWithDocs } from "../../utils/data.ts";
import { PackageNav, Params } from "./(_components)/PackageNav.tsx";
import { PackageHeader } from "./(_components)/PackageHeader.tsx";
import { DocsView } from "./(_components)/Docs.tsx";
import { scopeIAM } from "../../utils/iam.ts";

interface Data {
  package: Package;
  selectedVersion: PackageVersionWithUser | null;
  docs: Docs | null;
  member: ScopeMember | null;
}

export default function PackagePage(
  { data, params, state }: PageProps<Data, State>,
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
          <div class="mt-8 text-gray-500 text-center">
            This package has not published{" "}
            {data.package.versionCount > 0
              ? "a stable release"
              : "any versions"} yet.
          </div>
        )}
    </div>
  );
}

export const handler: Handlers<Data, State> = {
  async GET(_, ctx) {
    const res = await packageDataWithDocs(
      ctx.state,
      ctx.params.scope,
      ctx.params.package,
      ctx.params.version,
      {},
    );
    if (res === null) return ctx.renderNotFound();
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

    return ctx.render({
      package: pkg,
      selectedVersion,
      docs,
      member: scopeMember,
    }, {
      headers: { ...(ctx.params.version ? { "X-Robots-Tag": "noindex" } : {}) },
    });
  },
};

export const config: RouteConfig = {
  routeOverride: "/@:scope/:package{@:version}?",
};
