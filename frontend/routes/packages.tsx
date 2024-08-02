// Copyright 2024 the JSR authors. All rights reserved. MIT license.
import { Handlers, PageProps } from "$fresh/server.ts";
import { Head } from "$fresh/runtime.ts";
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
      <Head>
        <title>
          {data.query ? `${data.query} - Packages` : "Explore Packages"} - JSR
        </title>
        <meta
          name="description"
          content="JSR is the open-source package registry for modern JavaScript. JSR natively supports TypeScript, and works with all JS runtimes and package managers."
        />
        <meta property="og:image" content="/images/og-image.webp" />
      </Head>
      <div>
        <ListDisplay
          title={`${data.query ? "Search" : "Explore"} Packages`}
          pagination={data}
          currentUrl={url}
        >
          {data.packages.map((entry) => PackageHit(entry))}
        </ListDisplay>

        <div className="mt-2 flex flex-wrap items-start justify-between px-2">
          <span className="text-sm text-jsr-gray-400 block">
            Changes made in the last 15 minutes may not be visible yet. Packages
            with no published versions are not shown.
          </span>
          <div class="flex items-center gap-1">
            <span className="text-sm text-jsr-gray-500">powered by</span>
            <span className="sr-only">Orama</span>
            <img className="h-4" src="/logos/orama-dark.svg" alt="" />
          </div>
        </div>
      </div>
    </div>
  );
}

const apiKey = Deno.env.get("ORAMA_PACKAGE_PUBLIC_API_KEY");
const indexId = Deno.env.get("ORAMA_PACKAGE_PUBLIC_INDEX_ID");

export const handler: Handlers<Data, State> = {
  async GET(_req, ctx) {
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

      packages = res?.hits.map((hit) =>
        hit.document
      ).filter((document) => document) ?? [];
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

    return ctx.render({
      packages,
      query: search,
      page,
      limit,
      total,
    });
  },
};
