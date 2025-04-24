// Copyright 2024 the JSR authors. All rights reserved. MIT license.
import { HttpError, type RouteConfig } from "fresh";
import type { Dependency } from "../../../utils/api_types.ts";
import { path } from "../../../utils/api.ts";
import { define } from "../../../util.ts";
import { packageDataWithVersion } from "../../../utils/data.ts";
import { PackageHeader } from "../(_components)/PackageHeader.tsx";
import { PackageNav, type Params } from "../(_components)/PackageNav.tsx";
import { Table, TableData, TableRow } from "../../../components/Table.tsx";
import { scopeIAM } from "../../../utils/iam.ts";

function getDependencyLink(dep: Dependency) {
  if (dep.kind === "jsr") {
    return `/${dep.name}`;
  }
  const result = /^@jsr\/(?<scope>[a-z0-9-]+)__(?<package>[a-z0-9-]+)/
    .exec(
      dep.name,
    );
  if (result?.groups?.scope && result?.groups?.package) {
    return `/@${result.groups.scope}/${result.groups.package}`;
  }
  return `https://www.npmjs.com/package/${dep.name}`;
}

export default define.page<typeof handler>(function Deps(
  { data, params, state, url },
) {
  const iam = scopeIAM(state, data.member);

  const deps: Record<
    string,
    {
      link: string;
      constraints: Set<string>;
      modules: Record<string, string | undefined>;
      defaultModule: boolean;
    }
  > = {};

  for (const dep of data.deps) {
    const key = `${dep.kind}:${dep.name}`;
    deps[key] ??= {
      link: getDependencyLink(dep),
      constraints: new Set(),
      modules: {},
      defaultModule: false,
    };
    deps[key].constraints.add(dep.constraint);
    if (dep.path) {
      deps[key].modules[dep.path] = dep.kind === "jsr"
        ? `/${dep.name}/doc/${dep.path}/~`
        : undefined;
    } else {
      deps[key].defaultModule = true;
    }
  }

  const list = Object.entries(deps);

  return (
    <div class="mb-20">
      <PackageHeader
        package={data.package}
        selectedVersion={data.selectedVersion}
        downloads={data.downloads}
      />

      <PackageNav
        currentTab="Dependencies"
        versionCount={data.package.versionCount}
        dependencyCount={data.package.dependencyCount}
        dependentCount={data.package.dependentCount}
        iam={iam}
        params={params as unknown as Params}
        latestVersion={data.package.latestVersion}
      />

      <div class="space-y-3 mt-8">
        {list.length === 0
          ? (
            <div class="text-tertiary text-lg text-center">
              @{data.package.scope}/{data.package.name}@{data.selectedVersion
                .version} has no dependencies. 🎉
            </div>
          )
          : (
            <>
              <Table
                columns={[
                  { title: "Package", class: "w-1/3" },
                  { title: "Versions", class: "w-1/3" },
                  { title: "Modules", class: "w-auto" },
                ]}
                currentUrl={url}
              >
                {list.map(([name, info]) => (
                  <Dependency
                    name={name}
                    link={info.link}
                    constraints={[...info.constraints]}
                    modules={Object.entries(info.modules)}
                    defaultModule={info.defaultModule}
                  />
                ))}
              </Table>
              <p class="text-jsr-gray-700 dark:text-jsr-gray-200">
                You can find a visualization of the dependencies by clicking the
                button below.
              </p>
              <a class="button-primary" href={`${url.pathname}/graph`}>
                Dependency Graph
              </a>
            </>
          )}
      </div>
    </div>
  );
});

function Dependency(
  { name, link, constraints, modules, defaultModule }: {
    name: string;
    link: string;
    constraints: string[];
    modules: [path: string, link?: string][];
    defaultModule: boolean;
  },
) {
  return (
    <TableRow key={name}>
      <TableData>
        <a href={link} class="link">
          {name}
        </a>
      </TableData>
      <TableData class="space-x-4">
        {constraints.map((constraint, idx) => (
          <span key={idx}>{constraint}</span>
        ))}
      </TableData>
      <TableData>
        {modules.length > 0 && (
          <ul>
            {defaultModule && <li class="italic">(default)</li>}
            {modules.map(([path, link]) => (
              <li>
                {link
                  ? (
                    <a href={link} class="link">
                      {path}
                    </a>
                  )
                  : (
                    <span>
                      {path}
                    </span>
                  )}
              </li>
            ))}
          </ul>
        )}
      </TableData>
    </TableRow>
  );
}

export const handler = define.handlers({
  async GET(ctx) {
    const res = await packageDataWithVersion(
      ctx.state,
      ctx.params.scope,
      ctx.params.package,
      ctx.params.version,
    );
    if (res === null) {
      throw new HttpError(
        404,
        "This package or this package version was not found.",
      );
    }

    const {
      pkg,
      downloads,
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

    ctx.state.meta = {
      title: `Dependencies - @${pkg.scope}/${pkg.name} - JSR`,
      description: `@${pkg.scope}/${pkg.name} on JSR${
        pkg.description ? `: ${pkg.description}` : ""
      }`,
    };
    return {
      data: {
        package: pkg,
        downloads,
        deps: depsResp.data,
        selectedVersion,
        member: scopeMember,
      },
      headers: { "X-Robots-Tag": "noindex" },
    };
  },
});

export const config: RouteConfig = {
  routeOverride: "/@:scope/:package{@:version}?/dependencies",
};
