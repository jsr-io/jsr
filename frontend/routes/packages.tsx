// Copyright 2024 the JSR authors. All rights reserved. MIT license.
import { define } from "../util.ts";
import { OramaCloud } from "@orama/core";
import type { List, Package } from "../utils/api_types.ts";
import { assertOk, path } from "../utils/api.ts";
import { ListDisplay } from "../components/List.tsx";
import { PackageHit } from "../components/PackageHit.tsx";
import { processFilter } from "../islands/GlobalSearch.tsx";

export default define.page<typeof handler>(function PackageListPage({
  data,
  url,
}) {
  return (
    <div class="mb-24 space-y-16">
      <div>
        <ListDisplay
          title={`${data.query ? "Search" : "Explore"} Packages`}
          pagination={data}
          currentUrl={url}
        >
          {data.packages.map((entry) => PackageHit(entry))}
        </ListDisplay>

        <div className="mt-2 flex flex-wrap items-start justify-between px-2">
          <span className="text-sm text-jsr-gray-400 dark:text-jsr-gray-400 block">
            Changes made in the last 15 minutes may not be visible yet. Packages
            with no published versions are not shown.
          </span>
          <div class="flex items-center gap-1">
            <span className="text-sm text-tertiary">powered by</span>
            <span className="sr-only">Orama</span>
            <img
              className="h-4 dark:hidden"
              src="/logos/orama-dark.svg"
              alt=""
            />
            <img
              className="h-4 hidden dark:block"
              src="/logos/orama-light.svg"
              alt=""
            />
          </div>
        </div>
      </div>
    </div>
  );
});

const projectId = process.env.ORAMA_PACKAGES_PROJECT_ID;
const apiKey = process.env.ORAMA_PACKAGES_PUBLIC_API_KEY;

export const handler = define.handlers({
  async GET(ctx) {
    const search = ctx.url.searchParams.get("search") || "";
    const page = +(ctx.url.searchParams.get("page") || 1);
    const limit = +(ctx.url.searchParams.get("limit") || 20);

    let packages: Package[];
    let total;
    if (apiKey) {
      const orama = new OramaCloud({
        projectId: projectId!,
        apiKey: apiKey!,
      });

      const { query, where } = processFilter(search);

      const res = await orama.search({
        term: query,
        where,
        limit,
        offset: (page - 1) * limit,
        mode: "fulltext",
        boost: {
          id: 3,
          scope: 2,
          name: 1,
          description: 0.5,
        },
      });

      packages = res?.hits
        // deno-lint-ignore no-explicit-any
        .map((hit) => hit.document).filter((document) => document) as any ?? [];
      total = res?.count ?? 0;

      if (total === 0 && search !== "") {
        const fallback = await getPackagesFromApiFallback(
          ctx,
          search,
          page,
          limit,
        );
        if (fallback !== null) {
          packages = fallback.packages;
          total = fallback.total;
        }
      }
    } else {
      const packagesResp = await ctx.state.api.get<List<Package>>(
        path`/packages`,
        {
          query: search,
          page,
          limit,
        },
      );
      assertOk(packagesResp);

      packages = packagesResp.data.items;
      total = packagesResp.data.total;
    }

    ctx.state.meta = {
      title: search ? `${search} - Packages` : "Explore Packages",
      description:
        "JSR is the open-source package registry for modern JavaScript. JSR natively supports TypeScript, and works with all JS runtimes and package managers.",
    };
    return {
      data: {
        packages,
        query: search,
        page,
        limit,
        total,
      },
    };
  },
});

async function getPackagesFromApiFallback(
  ctx: Parameters<typeof handler.GET>[0],
  search: string,
  page: number,
  limit: number,
): Promise<{ packages: Package[]; total: number } | null> {
  const { query, where } = processFilter(search);

  if (
    where && Object.keys(where).length === 1 && typeof where.scope === "string"
  ) {
    const packagesResp = await ctx.state.api.get<List<Package>>(
      path`/scopes/${where.scope}/packages`,
      { limit: 100 },
    );
    assertOk(packagesResp);

    const packages = packagesResp.data.items.filter(isSearchablePackage);
    return paginatePackages(packages, page, limit);
  }

  if (where) return null;

  const packagesResp = await ctx.state.api.get<List<Package>>(
    path`/packages`,
    {
      query,
      limit: 100,
    },
  );
  assertOk(packagesResp);

  const packages = packagesResp.data.items.filter(isSearchablePackage);
  return paginatePackages(packages, page, limit);
}

function isSearchablePackage(pkg: Package): boolean {
  return pkg.versionCount > 0 && !pkg.isArchived &&
    !pkg.description.startsWith("INTERNAL");
}

function paginatePackages(
  packages: Package[],
  page: number,
  limit: number,
): { packages: Package[]; total: number } {
  const offset = (page - 1) * limit;
  return {
    packages: packages.slice(offset, offset + limit),
    total: packages.length,
  };
}
