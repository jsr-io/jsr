// Copyright 2024 the JSR authors. All rights reserved. MIT license.
import { define } from "../util.ts";
import { liteClient } from "algoliasearch/lite";
import type { List, Package } from "../utils/api_types.ts";
import { assertOk, path } from "../utils/api.ts";
import { ListDisplay } from "../components/List.tsx";
import { PackageHit } from "../components/PackageHit.tsx";
import { processFilter } from "../islands/GlobalSearch.tsx";
import SearchInsights from "../islands/SearchInsights.tsx";

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

        {data.queryID && (
          <SearchInsights
            appId={appId}
            apiKey={apiKey}
            index={indexName}
            queryID={data.queryID}
            hits={data.packages.map((entry, i) => ({
              // deno-lint-ignore no-explicit-any
              objectID: (entry as any).objectID ??
                `@${entry.scope}/${entry.name}`,
              href: `/@${entry.scope}/${entry.name}`,
              position: (data.page - 1) * data.limit + i + 1,
            }))}
          />
        )}

        <div className="mt-2 flex flex-wrap items-start justify-between px-2">
          <span className="text-sm text-jsr-gray-400 dark:text-jsr-gray-400 block">
            Changes made in the last 15 minutes may not be visible yet. Packages
            with no published versions are not shown.
          </span>
          <div class="flex items-center gap-1">
            <span className="text-sm text-tertiary">powered by</span>
            <a
              href="https://www.algolia.com/?utm_medium=AOS-referral"
              target="_blank"
              aria-label="Algolia"
            >
              <img class="h-4" src="/logos/algolia.svg" alt="Algolia" />
            </a>
          </div>
        </div>
      </div>
    </div>
  );
});

const appId = process.env.ALGOLIA_APP_ID;
const apiKey = process.env.ALGOLIA_PACKAGES_SEARCH_API_KEY;
const indexName = process.env.ALGOLIA_PACKAGES_INDEX;

export const handler = define.handlers({
  async GET(ctx) {
    const search = ctx.url.searchParams.get("search") || "";
    const page = +(ctx.url.searchParams.get("page") || 1);
    const limit = +(ctx.url.searchParams.get("limit") || 20);

    let packages: Package[];
    let total;
    let queryID: string | undefined;
    if (appId && apiKey && indexName) {
      const algolia = liteClient(appId, apiKey);

      const { query, filters } = processFilter(search);

      const { results } = await algolia.search({
        requests: [{
          indexName,
          query,
          filters,
          hitsPerPage: limit,
          page: page - 1,
          clickAnalytics: true,
        }],
      });

      // deno-lint-ignore no-explicit-any
      const result = results[0] as any;
      packages = result?.hits ?? [];
      total = result?.nbHits ?? 0;
      queryID = result?.queryID;
    } else {
      const packagesResp = await ctx.state.api.get<List<Package>>(
        path`/packages`,
        {
          search,
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
        queryID,
      },
    };
  },
});
