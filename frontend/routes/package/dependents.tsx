// Copyright 2024 the JSR authors. All rights reserved. MIT license.
import { HttpError, RouteConfig } from "fresh";
import { define } from "../../util.ts";
import type { Dependent, List } from "../../utils/api_types.ts";
import { path } from "../../utils/api.ts";
import { packageData } from "../../utils/data.ts";
import { PackageHeader } from "./(_components)/PackageHeader.tsx";
import { PackageNav, Params } from "./(_components)/PackageNav.tsx";
import { Table, TableData, TableRow } from "../../components/Table.tsx";
import { scopeIAM } from "../../utils/iam.ts";

export default define.page<typeof handler>(function Dep(
  { data, params, state, url },
) {
  const iam = scopeIAM(state, data.member);

  return (
    <div class="mb-20">
      <PackageHeader package={data.package} />

      <PackageNav
        currentTab="Dependents"
        versionCount={data.package.versionCount}
        iam={iam}
        params={params as unknown as Params}
        latestVersion={data.package.latestVersion}
      />

      <div class="space-y-4 mt-8">
        {data.dependents.length === 0
          ? (
            <div class="text-jsr-gray-500 text-center">
              This package is not depended on by any other JSR packages.
            </div>
          )
          : (
            <Table
              columns={[
                { title: "Name", class: "w-1/3" },
                { title: "Versions", class: "w-auto" },
              ]}
              pagination={data}
              currentUrl={url}
            >
              {data.dependents.map((dependent) => (
                <Dependent
                  scope={dependent.scope}
                  package={dependent.package}
                  versions={dependent.versions}
                  totalVersions={dependent.totalVersions}
                />
              ))}
            </Table>
          )}
      </div>
    </div>
  );
});

function Dependent(
  { scope, package: pkg, versions, totalVersions }: {
    scope: string;
    package: string;
    versions: string[];
    totalVersions: number;
  },
) {
  const name = `jsr:@${scope}/${pkg}`;
  return (
    <TableRow key={name}>
      <TableData>
        <a href={`/@${scope}/${pkg}`} class="link">
          {name}
        </a>
      </TableData>
      <TableData class="space-x-4">
        {versions.map((version) => <span>{version}</span>)}
        {totalVersions > 5 && (
          <span>
            and {totalVersions - 5} additional version{totalVersions > 6 && "s"}
          </span>
        )}
      </TableData>
    </TableRow>
  );
}

export const handler = define.handlers({
  async GET(ctx) {
    const page = +(ctx.url.searchParams.get("page") || 1);
    const limit = +(ctx.url.searchParams.get("limit") || 20);

    const [res, dependentsResp] = await Promise.all([
      packageData(ctx.state, ctx.params.scope, ctx.params.package),
      ctx.state.api.get<List<Dependent>>(
        path`/scopes/${ctx.params.scope}/packages/${ctx.params.package}/dependents`,
        { page, limit },
      ),
    ]);
    if (res === null) throw new HttpError(404, "This package was not found.");

    // TODO: handle errors gracefully
    if (!dependentsResp.ok) throw dependentsResp;

    ctx.state.meta = {
      title: `Dependents - @${res.pkg.scope}/${res.pkg.name} - JSR`,
      description: `@${res.pkg.scope}/${res.pkg.name} on JSR${
        res.pkg.description ? `: ${res.pkg.description}` : ""
      }`,
    };
    return {
      data: {
        package: res.pkg,
        dependents: dependentsResp.data.items,
        member: res.scopeMember,
        page,
        limit,
        total: dependentsResp.data.total,
      },
      headers: { "X-Robots-Tag": "noindex" },
    };
  },
});

export const config: RouteConfig = {
  routeOverride: "/@:scope/:package/dependents",
};
