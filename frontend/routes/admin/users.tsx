// Copyright 2024 the JSR authors. All rights reserved. MIT license.
import { Table, TableData, TableRow } from "../../components/Table.tsx";
import { path } from "../../utils/api.ts";
import { FullUser, List } from "../../utils/api_types.ts";
import { AdminNav } from "./(_components)/AdminNav.tsx";
import { URLQuerySearch } from "./(_components)/URLQuerySearch.tsx";
import { define } from "../../util.ts";
import twas from "twas";
import { AdminCopyButton } from "./(_islands)/AdminCopyButton.tsx";
import { EditModal } from "./(_islands)/EditModal.tsx";

export default define.page<typeof handler>(function Users({ data, url }) {
  return (
    <div class="mb-20">
      <AdminNav currentTab="users" />
      <URLQuerySearch query={data.query} />
      <Table
        class="mt-8"
        columns={[
          { title: "Name", class: "w-0", fieldName: "name" },
          { title: "E-Mail", class: "w-0", fieldName: "email" },
          { title: "GitHub ID", class: "w-0", fieldName: "github_id" },
          { title: "Scope Limit", class: "w-0", fieldName: "scope_limit" },
          { title: "Is Staff", class: "w-0", fieldName: "is_staff" },
          { title: "Is Blocked", class: "w-0", fieldName: "is_blocked" },
          {
            title: "Created",
            class: "w-0",
            fieldName: "created_at",
            align: "right",
          },
          { title: "", class: "w-0" },
        ]}
        pagination={data}
        sortBy={data.sortBy}
        currentUrl={url}
      >
        {data.users.map((user) => (
          <TableRow key={user.id}>
            <TableData flex>
              <AdminCopyButton value={user.id} label="copy user ID">
                ID
              </AdminCopyButton>
              <a href={`/user/${user.id}`} class="underline underline-offset-2">
                {user.name}
              </a>
            </TableData>
            <TableData>
              {user.email}
            </TableData>
            <TableData>
              {user.githubId}
            </TableData>
            <TableData>
              {user.scopeLimit}
            </TableData>
            <TableData>
              {String(user.isStaff)}
            </TableData>
            <TableData>
              {String(user.isBlocked)}
            </TableData>
            <TableData
              title={new Date(user.createdAt).toISOString().slice(0, 10)}
              align="right"
            >
              {twas(new Date(user.createdAt).getTime())}
            </TableData>
            <TableData align="right">
              <EditModal
                style="primary"
                path={path`/admin/users/${user.id}`}
                title={`Edit user '${user.name}'`}
                fields={[
                  {
                    name: "scopeLimit",
                    label: "scope limit",
                    type: "number",
                    value: user.scopeLimit,
                  },
                  {
                    name: "isStaff",
                    label: "is staff",
                    type: "boolean",
                    value: user.isStaff,
                  },
                  {
                    name: "isBlocked",
                    label: "is blocked",
                    type: "boolean",
                    value: user.isBlocked,
                  },
                ]}
              />
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
    const sortBy = ctx.url.searchParams.get("sortBy") || "";
    const page = +(ctx.url.searchParams.get("page") || 1);
    const limit = +(ctx.url.searchParams.get("limit") || 20);

    const resp = await ctx.state.api.get<List<FullUser>>(path`/admin/users`, {
      query,
      sortBy,
      page,
      limit,
    });
    if (!resp.ok) throw resp; // gracefully handle this

    return {
      data: {
        users: resp.data.items,
        query,
        sortBy,
        page,
        limit,
        total: resp.data.total,
      },
    };
  },
});
