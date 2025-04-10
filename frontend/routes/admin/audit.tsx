// Copyright 2024 the JSR authors. All rights reserved. MIT license.
import { Table, TableData, TableRow } from "../../components/Table.tsx";
import { path } from "../../utils/api.ts";
import { AuditLog, List } from "../../utils/api_types.ts";
import { AdminNav } from "./(_components)/AdminNav.tsx";
import { URLQuerySearch } from "../../components/URLQuerySearch.tsx";
import { define } from "../../util.ts";
import twas from "twas";

export default define.page<typeof handler>(function Users({ data, url }) {
  return (
    <div class="mb-20">
      <AdminNav currentTab="audit" />
      <URLQuerySearch query={data.query} />
      <Table
        class="mt-8"
        columns={[
          { title: "Action", class: "w-0" },
          { title: "User", class: "w-0" },
          { title: "Meta", class: "w-0" },
          { title: "Created", class: "w-0" },
        ]}
        pagination={data}
        currentUrl={url}
      >
        {data.logs.map((log, idx) => (
          <TableRow key={idx}>
            <TableData>
              {log.action.replaceAll("_", " ")}
            </TableData>
            <TableData>
              <a href={`/users/${log.user.id}`}>{log.user.name}</a>
            </TableData>
            <TableData>
              {JSON.stringify(log.meta, null, " ")}
            </TableData>
            <TableData
              title={new Date(log.createdAt).toISOString().slice(
                0,
                10,
              )}
            >
              {twas(new Date(log.createdAt).getTime())}
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
    const page = +(ctx.url.searchParams.get("page") || 1);
    const limit = +(ctx.url.searchParams.get("limit") || 20);

    const resp = await ctx.state.api.get<List<AuditLog>>(
      path`/admin/audit_logs`,
      {
        query,
        page,
        limit,
      },
    );
    if (!resp.ok) throw resp; // gracefully handle this

    return {
      data: {
        logs: resp.data.items,
        query,
        page,
        limit,
        total: resp.data.total,
      },
    };
  },
});
