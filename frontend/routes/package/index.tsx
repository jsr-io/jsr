// Copyright 2024 the JSR authors. All rights reserved. MIT license.
import { Handlers, PageProps, RouteConfig } from "$fresh/server.ts";
import type { Package, PackageVersionWithUser } from "../../utils/api_types.ts";
import { Docs, State } from "../../util.ts";
import { ScopeMember } from "../../utils/api_types.ts";
import { Head } from "$fresh/src/runtime/head.ts";
import { packageDataWithDocs } from "../../utils/data.ts";
import { PackageNav, Params } from "./(_components)/PackageNav.tsx";
import { PackageHeader } from "./(_components)/PackageHeader.tsx";
import { DocsView } from "./(_components)/Docs.tsx";

interface Data {
  package: Package;
  selectedVersion: PackageVersionWithUser | null;
  docs: Docs | null;
  member: ScopeMember | null;
}

export default function PackagePage(
  { data, params, state }: PageProps<Data, State>,
) {
  const isStaff = state.user?.isStaff || false;
  const canEdit = data.member?.isAdmin || isStaff;

  return (
    <div class="mb-20">
      <Head>
        <title>
          @{params.scope}/{params.package} - JSR
        </title>
      </Head>

      <PackageHeader
        package={data.package}
        selectedVersion={data.selectedVersion ?? undefined}
      />
      <PackageNav
        currentTab="Index"
        versionCount={data.package.versionCount}
        canEdit={canEdit}
        params={params as unknown as Params}
        latestVersion={data.package.latestVersion}
      />

      {data.docs
        ? (
          <DocsView
            docs={data.docs}
            params={params as unknown as Params}
            selectedVersion={data.selectedVersion ?? undefined}
          />
        )
        : (
          <div class="mt-8 text-gray-500 text-center">
            This package has not published any versions yet.
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

    const {
      pkg,
      scopeMember,
      selectedVersion,
      docs,
    } = res;

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
