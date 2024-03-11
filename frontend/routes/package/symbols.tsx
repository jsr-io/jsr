// Copyright 2024 the JSR authors. All rights reserved. MIT license.
import { Handlers, PageProps, RouteConfig } from "$fresh/server.ts";
import { Head } from "$fresh/runtime.ts";
import type { Package, PackageVersionWithUser } from "../../utils/api_types.ts";
import { Docs, State } from "../../util.ts";
import { ScopeMember } from "../../utils/api_types.ts";
import { packageDataWithDocs } from "../../utils/data.ts";
import { PackageHeader } from "./(_components)/PackageHeader.tsx";
import { PackageNav, Params } from "./(_components)/PackageNav.tsx";
import { DocsView } from "./(_components)/Docs.tsx";
import { scopeIAM } from "../../utils/iam.ts";

interface Data {
  package: Package;
  docs: Docs;
  member: ScopeMember | null;
  selectedVersion: PackageVersionWithUser;
}

export default function Symbols(
  { data, params, state }: PageProps<Data, State>,
) {
  const iam = scopeIAM(state, data.member);

  return (
    <div class="mb-20">
      <Head>
        <title>
          All symbols - @{params.scope}/{params.package} - JSR
        </title>
        <meta
          name="description"
          content={`@${params.scope}/${params.package} on JSR${
            data.package.description ? `: ${data.package.description}` : ""
          }`}
        />
      </Head>

      <PackageHeader
        package={data.package}
        selectedVersion={data.selectedVersion}
      />

      <PackageNav
        currentTab="Symbols"
        versionCount={data.package.versionCount}
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
}

export const handler: Handlers<Data, State> = {
  async GET(_, ctx) {
    const res = await packageDataWithDocs(
      ctx.state,
      ctx.params.scope,
      ctx.params.package,
      ctx.params.version,
      { all_symbols: "true" },
    );
    if (!res) return ctx.renderNotFound();

    const {
      pkg,
      scopeMember,
      selectedVersion,
      docs,
    } = res;
    if (selectedVersion === null || docs === null) {
      return new Response(null, {
        status: 302,
        headers: {
          Location: `/@${ctx.params.scope}/${ctx.params.package}`,
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
  routeOverride: "/@:scope/:package{@:version}?/doc",
};
