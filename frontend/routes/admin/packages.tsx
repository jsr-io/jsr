// Copyright 2024 the JSR authors. All rights reserved. MIT license.
import { Table, TableData, TableRow } from "../../components/Table.tsx";
import { path } from "../../utils/api.ts";
import { List, Package } from "../../utils/api_types.ts";
import { AdminNav } from "./(_components)/AdminNav.tsx";
import { URLQuerySearch } from "./(_components)/URLQuerySearch.tsx";
import { define } from "../../util.ts";
import twas from "twas";
import { CopyButton } from "./(_islands)/CopyButton.tsx";

export default define.page<typeof handler>(function Packages({ data, url }) {
  return (
    <div class="mb-20">
      <AdminNav currentTab="packages" />
      <URLQuerySearch query={data.query} />
      <Table
        class="mt-8"
        columns={[
          { title: "Scope", class: "w-0" },
          { title: "Name", class: "w-0" },
          { title: "Repository", class: "w-0" },
          { title: "Archived", class: "w-0" },
          { title: "Featured", class: "w-0" },
          { title: "Updated", class: "w-0" },
          { title: "Created", class: "w-0" },
        ]}
        pagination={data}
        sortBy={data.sortBy}
        currentUrl={url}
      >
        {data.packages.map((pkg, i) => (
          <TableRow key={i}>
            <TableData>
              <a
                href={`/admin/scopes?search=${pkg.scope}`}
                class="underline underline-offset-2"
              >
                {pkg.scope}
              </a>
            </TableData>
            <TableData>
              <a
                href={`/@${pkg.scope}/${pkg.name}`}
                class="underline underline-offset-2"
              >
                {pkg.name}
              </a>
            </TableData>
            <TableData>
              {pkg.githubRepository && (
                <>
                  <CopyButton
                    value={pkg.githubRepository.id.toString()}
                    label="copy GitHub ID"
                  >
                    ID
                  </CopyButton>
                  <a
                    href={`github.com/${pkg.githubRepository.owner}/${pkg.githubRepository.name}`}
                    class="underline underline-offset-2"
                  >
                    {pkg.githubRepository.owner}/{pkg.githubRepository.name}
                  </a>
                </>
              )}
            </TableData>
            <TableData>
              {String(pkg.isArchived)}
            </TableData>
            <TableData
              title={pkg.whenFeatured
                ? new Date(pkg.whenFeatured).toISOString().slice(0, 10)
                : ""}
            >
              {pkg.whenFeatured && twas(new Date(pkg.whenFeatured).getTime())}
            </TableData>
            <TableData
              title={new Date(pkg.updatedAt).toISOString().slice(0, 10)}
            >
              {twas(new Date(pkg.updatedAt).getTime())}
            </TableData>
            <TableData
              title={new Date(pkg.createdAt).toISOString().slice(0, 10)}
            >
              {twas(new Date(pkg.createdAt).getTime())}
            </TableData>
          </TableRow>
        ))}
      </Table>
    </div>
  );
});

export const handler = define.handlers({
  async GET(ctx) {
    const query = ctx.url.searchParams.get("search") || "";
    const sortBy = ctx.url.searchParams.get("sortBy") || "created_at";
    const page = +(ctx.url.searchParams.get("page") || 1);
    const limit = +(ctx.url.searchParams.get("limit") || 20);

    const resp = await ctx.state.api.get<List<Package>>(path`/admin/packages`, {
      query,
      page,
      limit,
      sortBy,
    });
    if (!resp.ok) throw resp; // gracefully handle this

    return {
      data: {
        packages: resp.data.items,
        query,
        sortBy,
        page,
        limit,
        total: resp.data.total,
      },
    };
  },
});
