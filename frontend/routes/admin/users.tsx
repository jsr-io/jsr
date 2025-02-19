// Copyright 2024 the JSR authors. All rights reserved. MIT license.
import UserEdit from "../../islands/admin/UserEdit.tsx";
import { Table } from "../../components/Table.tsx";
import { path } from "../../utils/api.ts";
import { FullUser, List } from "../../utils/api_types.ts";
import { AdminNav } from "./(_components)/AdminNav.tsx";
import { URLQuerySearch } from "../../components/URLQuerySearch.tsx";
import { define } from "../../util.ts";

export default define.page<typeof handler>(function Users({ data, url }) {
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
        {data.users.map((user, idx) => <UserEdit key={idx} user={user} />)}
      </Table>
    </div>
  );
});

export const handler = define.handlers({
  async GET(ctx) {
    const query = ctx.url.searchParams.get("search") || "";
    const page = +(ctx.url.searchParams.get("page") || 1);
    const limit = +(ctx.url.searchParams.get("limit") || 20);

    const resp = await ctx.state.api.get<List<FullUser>>(path`/admin/users`, {
      query,
      page,
      limit,
    });
    if (!resp.ok) throw resp; // gracefully handle this

    return {
      data: {
        users: resp.data.items,
        query,
        page,
        limit,
        total: resp.data.total,
      },
    };
  },
});
