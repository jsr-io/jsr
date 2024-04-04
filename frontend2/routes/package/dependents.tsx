// Copyright 2024 the JSR authors. All rights reserved. MIT license.
import { Handlers, PageProps, RouteConfig } from "$fresh";
import type {
  Dependent,
  List,
  Package,
  ScopeMember,
} from "../../utils/api_types.ts";
import { path } from "../../utils/api.ts";
import { PaginationData, State } from "../../util.ts";
import { packageData } from "../../utils/data.ts";
import { PackageHeader } from "./(_components)/PackageHeader.tsx";
import { PackageNav, Params } from "./(_components)/PackageNav.tsx";
import { Table, TableData, TableRow } from "../../components/Table.tsx";
import { scopeIAM } from "../../utils/iam.ts";

interface Data extends PaginationData {
  package: Package;
  dependents: Dependent[];
  member: ScopeMember | null;
}

export default function Dep(
  { data, params, state, url }: PageProps<Data, State>,
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
            <div class="text-gray-500 text-center">
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
}

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

export const handler: Handlers<Data, State> = {
  async GET(req, ctx) {
    const reqUrl = new URL(req.url);
    const page = +(reqUrl.searchParams.get("page") || 1);
    const limit = +(reqUrl.searchParams.get("limit") || 20);

    const [res, dependentsResp] = await Promise.all([
      packageData(ctx.state, ctx.params.scope, ctx.params.package),
      ctx.state.api.get<List<Dependent>>(
        path`/scopes/${ctx.params.scope}/packages/${ctx.params.package}/dependents`,
        { page, limit },
      ),
    ]);
    if (res === null) return ctx.renderNotFound();

    // TODO: handle errors gracefully
    if (!dependentsResp.ok) throw dependentsResp;

    return ctx.render({
      package: res.pkg,
      dependents: dependentsResp.data.items,
      member: res.scopeMember,
      page,
      limit,
      total: dependentsResp.data.total,
    }, { headers: { "X-Robots-Tag": "noindex" } });
  },
};

export const config: RouteConfig = {
  routeOverride: "/@:scope/:package/dependents",
};
