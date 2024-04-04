// Copyright 2024 the JSR authors. All rights reserved. MIT license.
import { Handlers, PageProps } from "$fresh";
import type { PaginationData, State } from "../../util.ts";
import type { FullScope, List } from "../../utils/api_types.ts";
import ScopeEdit from "../../islands/admin/ScopeEdit.tsx";
import { Table } from "../../components/Table.tsx";
import { path } from "../../utils/api.ts";
import { AdminNav } from "./(_components)/AdminNav.tsx";
import { URLQuerySearch } from "../../components/URLQuerySearch.tsx";

interface Data extends PaginationData {
  scopes: FullScope[];
  query: string;
}

export default function Scopes({ data, url }: PageProps<Data>) {
  return (
    <div class="mb-20">
      <AdminNav currentTab="scopes" />
      <URLQuerySearch query={data.query} />
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
        {data.scopes.map((scope) => <ScopeEdit scope={scope} />)}
      </Table>
    </div>
  );
}

export const handler: Handlers<Data, State> = {
  async GET(req, ctx) {
    const reqUrl = new URL(req.url);
    const query = reqUrl.searchParams.get("search") || "";
    const page = +(reqUrl.searchParams.get("page") || 1);
    const limit = +(reqUrl.searchParams.get("limit") || 20);

    const resp = await ctx.state.api.get<List<FullScope>>(path`/admin/scopes`, {
      query,
      page,
      limit,
    });
    if (!resp.ok) throw resp; // gracefully handle this

    return ctx.render({
      scopes: resp.data.items,
      query,
      page,
      limit,
      total: resp.data.total,
    });
  },
};
