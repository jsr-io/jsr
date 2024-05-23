// Copyright 2024 the JSR authors. All rights reserved. MIT license.
import { Handlers, PageProps, RouteConfig } from "$fresh/server.ts";
import type {
  Dependency,
  Package,
  PackageVersionWithUser,
  ScopeMember,
} from "../../utils/api_types.ts";
import { path } from "../../utils/api.ts";
import { State } from "../../util.ts";
import { packageDataWithVersion } from "../../utils/data.ts";
import { PackageHeader } from "./(_components)/PackageHeader.tsx";
import { PackageNav, Params } from "./(_components)/PackageNav.tsx";
import { Table, TableData, TableRow } from "../../components/Table.tsx";
import { Head } from "$fresh/runtime.ts";
import { scopeIAM } from "../../utils/iam.ts";

interface Data {
  package: Package;
  deps: Dependency[];
  selectedVersion: PackageVersionWithUser;
  member: ScopeMember | null;
}

export default function Deps(
  { data, params, state, url }: PageProps<Data, State>,
) {
  const iam = scopeIAM(state, data.member);

  const deps: Record<
    string,
    {
      packageName: string;
      packageLink: string;
      moduleName?: string;
      moduleLink?: string;
      constraints: Set<string>;
    }
  > = {};

  for (const dep of data.deps) {
    const key = `${dep.kind}:${dep.name}${dep.path ? `/${dep.path}` : ""}`;
    deps[key] ??= {
      packageName: `${dep.kind}:${dep.name}`,
      packageLink: `${
        dep.kind === "jsr" ? "/" : "https://www.npmjs.com/package/"
      }${dep.name}`,
      moduleName: dep.path,
      moduleLink: dep.path && dep.kind === "jsr"
        ? `/${dep.name}/doc/${dep.path}/~`
        : "",
      constraints: new Set(),
    };
    deps[key].constraints.add(dep.constraint);
  }

  const list = Object.entries(deps);

  return (
    <div class="mb-20">
      <Head>
        <title>
          Dependencies - @{params.scope}/{params.package} - JSR
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
        currentTab="Dependencies"
        versionCount={data.package.versionCount}
        iam={iam}
        params={params as unknown as Params}
        latestVersion={data.package.latestVersion}
      />

      <div class="space-y-4 mt-8">
        {list.length === 0
          ? (
            <div class="text-gray-500 text-center">
              @{data.package.scope}/{data.package.name}@{data.selectedVersion
                .version} has no dependencies. 🎉
            </div>
          )
          : (
            <Table
              columns={[
                { title: "Package / Module", class: "w-1/3" },
                { title: "Versions", class: "w-auto" },
              ]}
              currentUrl={url}
            >
              {list.map(([key, { constraints, ...info }]) => (
                <Dependency
                  key={key}
                  {...info}
                  constraints={[...constraints]}
                />
              ))}
            </Table>
          )}
      </div>
    </div>
  );
}

function Dependency(
  { packageName, packageLink, moduleName, moduleLink, constraints }: {
    packageName: string;
    packageLink: string;
    moduleName?: string;
    moduleLink?: string;
    constraints: string[];
  },
) {
  return (
    <TableRow>
      <TableData class="space-x-1">
        <a href={packageLink} class="link">
          {packageName}
        </a>
        {moduleName && (
          <>
            <span>/</span>
            {moduleLink
              ? (
                <a href={moduleLink} class="link">
                  {moduleName}
                </a>
              )
              : <span>{moduleName}</span>}
          </>
        )}
      </TableData>
      <TableData class="space-x-4">
        {constraints.map((constraint) => <span>{constraint}</span>)}
      </TableData>
    </TableRow>
  );
}

export const handler: Handlers<Data, State> = {
  async GET(_, ctx) {
    const res = await packageDataWithVersion(
      ctx.state,
      ctx.params.scope,
      ctx.params.package,
      ctx.params.version,
    );
    if (res === null) return ctx.renderNotFound();
    const {
      pkg,
      scopeMember,
      selectedVersion,
    } = res;

    if (selectedVersion === null) {
      return new Response(null, {
        status: 302,
        headers: {
          Location: `/@${ctx.params.scope}/${ctx.params.package}`,
        },
      });
    }

    const depsResp = await ctx.state.api.get<Dependency[]>(
      path`/scopes/${pkg.scope}/packages/${pkg.name}/versions/${selectedVersion.version}/dependencies`,
    );
    if (!depsResp.ok) throw depsResp;

    return ctx.render({
      package: pkg,
      deps: depsResp.data,
      selectedVersion,
      member: scopeMember,
    }, { headers: { "X-Robots-Tag": "noindex" } });
  },
};

export const config: RouteConfig = {
  routeOverride: "/@:scope/:package{@:version}?/dependencies",
};
