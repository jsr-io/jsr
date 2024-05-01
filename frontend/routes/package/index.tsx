// Copyright 2024 the JSR authors. All rights reserved. MIT license.
import { Handlers, PageProps, RouteConfig } from "$fresh/server.ts";
import type { Package, PackageVersionWithUser } from "../../utils/api_types.ts";
import { Docs, State } from "../../util.ts";
import { ScopeMember } from "../../utils/api_types.ts";
import { Head } from "$fresh/src/runtime/head.ts";
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
const FRONTEND_ROOT = Deno.env.get('FRONTEND_ROOT') ?? 'http://jsr.test'

export default function PackagePage(
  { data, params, state }: PageProps<Data, State>,
) {
  const iam = scopeIAM(state, data.member);

  const title = `@${params.scope}/${params.package} - JSR`
  const description = `@${params.scope}/${params.package} on JSR${
    data.package.description ? `: ${data.package.description}` : ""
  }`

  return (
    <div>
      <Head>
        <title>
          @{params.scope}/{params.package} - JSR
        </title>
        <meta
          name="description"
          content={`@${params.scope}/${params.package} on JSR${
            data.package.description ? `: ${data.package.description}` : ""
          }`}
        />
        <meta property="og:url" content={`${FRONTEND_ROOT}/@${params.scope}/${params.package}`} />
        <meta property="og:type" content="article" />
        <meta property="og:title" content={title} />
        <meta property="og:description" content={description} />
        <meta property="og:site_name" content="JSR" />
        <meta property="og:image" content={`${FRONTEND_ROOT}/@${params.scope}/${params.package}/og`} />
      </Head>

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
