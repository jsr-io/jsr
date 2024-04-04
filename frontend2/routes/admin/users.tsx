// Copyright 2024 the JSR authors. All rights reserved. MIT license.
import { Handlers, PageProps } from "$fresh";
import type { PaginationData, State } from "../../util.ts";
import UserEdit from "../../islands/admin/UserEdit.tsx";
import { Table } from "../../components/Table.tsx";
import { path } from "../../utils/api.ts";
import { FullUser, List } from "../../utils/api_types.ts";
import { AdminNav } from "./(_components)/AdminNav.tsx";
import { URLQuerySearch } from "../../components/URLQuerySearch.tsx";

interface Data extends PaginationData {
  users: FullUser[];
  query: string;
}

export default function Users({ data, url }: PageProps<Data>) {
  return (
    <div class="mb-20">
      <AdminNav currentTab="users" />
      <URLQuerySearch query={data.query} />
      <Table
        class="mt-8"
        columns={[
          { title: "Name", class: "w-auto" },
          { title: "E-Mail", class: "w-0" },
          { title: "GitHub ID", class: "w-0" },
          { title: "Scope Limit", class: "w-0" },
          { title: "Is Staff", class: "w-0" },
          { title: "Is Blocked", class: "w-0" },
          { title: "Created", class: "w-0" },
          { title: "", class: "w-0", align: "right" },
        ]}
        pagination={data}
        currentUrl={url}
      >
        {data.users.map((user) => <UserEdit user={user} />)}
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

    const resp = await ctx.state.api.get<List<FullUser>>(path`/admin/users`, {
      query,
      page,
      limit,
    });
    if (!resp.ok) throw resp; // gracefully handle this

    return ctx.render({
      users: resp.data.items,
      query,
      page,
      limit,
      total: resp.data.total,
    });
  },
};
