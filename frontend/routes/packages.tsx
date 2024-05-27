// Copyright 2024 the JSR authors. All rights reserved. MIT license.
import { Handlers, PageProps } from "@fresh/core";
import { OramaPackageHit, PaginationData, State } from "../util.ts";
import { OramaClient } from "@oramacloud/client";
import type { List, Package } from "../utils/api_types.ts";
import { path } from "../utils/api.ts";
import { ListDisplay } from "../components/List.tsx";
import { PackageHit } from "../components/PackageHit.tsx";
import { processFilter } from "../islands/GlobalSearch.tsx";

interface Data extends PaginationData {
  packages: OramaPackageHit[] | Package[];
  query: string;
}

export default function PackageListPage({
  data,
  url,
}: PageProps<Data>) {
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
          <span className="text-sm text-gray-400 block">
            Changes made in the last 15 minutes may not be visible yet. Packages
            with no published versions are not shown.
          </span>
          <div class="flex items-center gap-1">
            <span className="text-sm text-gray-500">powered by</span>
            <img className="h-4" src="/logos/orama-dark.svg" />
          </div>
        </div>
      </div>
    </div>
  );
}

const apiKey = Deno.env.get("ORAMA_PACKAGE_PUBLIC_API_KEY");
const indexId = Deno.env.get("ORAMA_PACKAGE_PUBLIC_INDEX_ID");

export const handler: Handlers<Data, State> = {
  async GET(ctx) {
    const search = ctx.url.searchParams.get("search") || "";
    const page = +(ctx.url.searchParams.get("page") || 1);
    const limit = +(ctx.url.searchParams.get("limit") || 20);

    let packages;
    let total;
    if (apiKey) {
      const orama = new OramaClient({
        endpoint: `https://cloud.orama.run/v1/indexes/${indexId!}`,
        api_key: apiKey,
      });

      const { query, where } = processFilter(search);

      const res = await orama.search({
        term: query,
        where,
        limit,
        offset: (page - 1) * limit,
        mode: "fulltext",
      });

      packages = res?.hits.map((hit) => hit.document) ?? [];
      total = res?.count ?? 0;
    } else {
      const packagesResp = await ctx.state.api.get<List<Package>>(
        path`/packages`,
        {
          search,
          page,
          limit,
        },
      );
      if (!packagesResp.ok) throw packagesResp; // gracefully handle this

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
};
