// Copyright 2024 the JSR authors. All rights reserved. MIT license.
import { Handlers, PageProps } from "$fresh/server.ts";
import { Head } from "$fresh/runtime.ts";
import { OramaPackageHit, PaginationData, State } from "../util.ts";
import { OramaClient } from "@oramacloud/client";
import type { List, Package } from "../utils/api_types.ts";
import {
  RuntimeCompatIndicator,
} from "../components/RuntimeCompatIndicator.tsx";
import { path } from "../utils/api.ts";
import { ListDisplay, ListDisplayItem } from "../components/List.tsx";

interface Data extends PaginationData {
  packages: OramaPackageHit[] | Package[];
  query: string;
}

export default function PackageListPage({ data, url }: PageProps<Data>) {
  return (
    <div class="mb-24 space-y-16">
      <Head>
        <title>
          {data.query ? `${data.query} - Packages` : "Explore Packages"} - JSR
        </title>
      </Head>
      <div>
        <ListDisplay
          title={`${data.query ? "Search" : "Explore"} Packages`}
          pagination={data}
          currentUrl={url}
        >
          {data.packages.map((entry) => ModuleHit(entry))}
        </ListDisplay>

        <div className="mt-2 flex items-start justify-between px-2">
          <span className="text-sm text-gray-400 block">
            Changes made in the last 15 minutes may not be visible yet.
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

export function ModuleHit(pkg: OramaPackageHit | Package): ListDisplayItem {
  return {
    href: `/@${pkg.scope}/${pkg.name}`,
    content: (
      <div class="grow-1 w-full flex flex-col md:flex-row gap-2 justify-between">
        <div class="grow-1">
          <div class="text-cyan-700 font-semibold">
            {`@${pkg.scope}/${pkg.name}`}
          </div>
          <div class="text-sm text-gray-600">
            {pkg.description}
          </div>
        </div>

        <RuntimeCompatIndicator
          runtimeCompat={pkg.runtimeCompat}
          hideUnknown
        />
      </div>
    ),
  };
}

const apiKey = Deno.env.get("ORAMA_PUBLIC_API_KEY");
const indexId = Deno.env.get("ORAMA_PUBLIC_INDEX_ID");

export const handler: Handlers<Data, State> = {
  async GET(req, ctx) {
    const reqUrl = new URL(req.url);
    const query = reqUrl.searchParams.get("search") || "";
    const page = +(reqUrl.searchParams.get("page") || 1);
    const limit = +(reqUrl.searchParams.get("limit") || 20);

    let packages;
    let total;
    if (apiKey) {
      const orama = new OramaClient({
        endpoint: `https://cloud.orama.run/v1/indexes/${indexId!}`,
        api_key: apiKey,
      });

      const res = await orama.search({
        term: query,
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
          query,
          page,
          limit,
        },
      );
      if (!packagesResp.ok) throw packagesResp; // gracefully handle this

      packages = packagesResp.data.items;
      total = packagesResp.data.total;
    }

    return ctx.render({
      packages,
      query,
      page,
      limit,
      total,
    });
  },
};
