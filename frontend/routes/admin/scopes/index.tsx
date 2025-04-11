// Copyright 2024 the JSR authors. All rights reserved. MIT license.
import type { FullScope, List } from "../../../utils/api_types.ts";
import ScopeEdit from "../../../islands/admin/ScopeEdit.tsx";
import { Table } from "../../../components/Table.tsx";
import { path } from "../../../utils/api.ts";
import { AdminNav } from "../(_components)/AdminNav.tsx";
import { URLQuerySearch } from "../(_components)/URLQuerySearch.tsx";
import { define } from "../../../util.ts";
import TbArrowRight from "tb-icons/TbArrowRight";

export default define.page<typeof handler>(function Scopes({ data, url }) {
  return (
    <div class="mb-20">
      <AdminNav currentTab="scopes" />
      <div class="flex gap-4">
        <URLQuerySearch query={data.query} />
        <a class="button-primary mt-4" href="/admin/scopes/assign">
          Assign Scope <TbArrowRight />
        </a>
      </div>
      <Table
        class="mt-8"
        columns={[
          { title: "Name", class: "w-auto" },
          { title: "Creator", class: "w-auto" },
          { title: "Pkgs", class: "w-0" },
          { title: "New Pkg", class: "w-0" },
          { title: "Pubs", class: "w-0" },
          { title: "Created", class: "w-0" },
          { title: "", class: "w-0", align: "right" },
        ]}
        pagination={data}
        currentUrl={url}
      >
        {data.scopes.map((scope, idx) => <ScopeEdit key={idx} scope={scope} />)}
      </Table>
    </div>
  );
});

export const handler = define.handlers({
  async GET(ctx) {
    const query = ctx.url.searchParams.get("search") || "";
    const page = +(ctx.url.searchParams.get("page") || 1);
    const limit = +(ctx.url.searchParams.get("limit") || 20);

    const resp = await ctx.state.api.get<List<FullScope>>(path`/admin/scopes`, {
      query,
      page,
      limit,
    });
    if (!resp.ok) throw resp; // gracefully handle this

    return {
      data: {
        scopes: resp.data.items,
        query,
        page,
        limit,
        total: resp.data.total,
      },
    };
  },
});
