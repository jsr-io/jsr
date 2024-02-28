// Copyright 2024 the JSR authors. All rights reserved. MIT license.
import { Handlers, PageProps, RouteConfig } from "$fresh/server.ts";
import { Head } from "$fresh/runtime.ts";
import type {
  Package,
  PackageVersionWithUser,
} from "../../../utils/api_types.ts";
import { Docs, State } from "../../../util.ts";
import { ScopeMember } from "../../../utils/api_types.ts";
import { packageDataWithDocs } from "../../../utils/data.ts";
import { PackageHeader } from "../(_components)/PackageHeader.tsx";
import { PackageNav, Params } from "../(_components)/PackageNav.tsx";
import { DocsView } from "../(_components)/Docs.tsx";

interface Data {
  package: Package;
  docs: Docs;
  member: ScopeMember | null;
  selectedVersion: PackageVersionWithUser;
}

export default function Symbol(
  { data, params, state }: PageProps<Data, State>,
) {
  const isStaff = state.user?.isStaff || false;
  const canEdit = data.member?.isAdmin || isStaff;

  return (
    <div class="mb-20">
      <Head>
        <title>
          {/* TODO: print symbol kind here (function / class / etc) */}
          {params.symbol}
          {params.entrypoint && ` from ${params.entrypoint}`}{" "}
          - @{params.scope}/{params.package} - JSR
        </title>
      </Head>

      <PackageHeader
        package={data.package}
        selectedVersion={data.selectedVersion}
      />

      <PackageNav
        currentTab="Index"
        versionCount={data.package.versionCount}
        canEdit={canEdit}
        params={params as unknown as Params}
        latestVersion={data.package.latestVersion}
      />

      <DocsView
        docs={data.docs}
        params={params as unknown as Params}
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
      {
        entrypoint: ctx.params.entrypoint,
        symbol: ctx.params.symbol,
      },
    );
    if (!res) return ctx.renderNotFound();

    const {
      pkg,
      scopeMember,
      selectedVersion,
      docs,
    } = res;
    if (selectedVersion === null) {
      return new Response(null, {
        status: 302,
        headers: {
          Location: `/@${ctx.params.scope}/${ctx.params.package}`,
        },
      });
    }

    if (!docs?.main) return ctx.renderNotFound();

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
  routeOverride: "/@:scope/:package{@:version}?/doc/:entrypoint*/~/:symbol+",
};
