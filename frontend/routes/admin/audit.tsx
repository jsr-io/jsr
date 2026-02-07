// Copyright 2024 the JSR authors. All rights reserved. MIT license.
import { Table, TableData, TableRow } from "../../components/Table.tsx";
import { assertOk, path } from "../../utils/api.ts";
import { AuditLog, List } from "../../utils/api_types.ts";
import { AdminNav } from "./(_components)/AdminNav.tsx";
import { AuditURLQuerySearch } from "./(_islands)/AuditURLQuerySearch.tsx";
import { define } from "../../util.ts";
import twas from "twas";

export default define.page<typeof handler>(function Users({ data, url }) {
  return (
    <div class="mb-20">
      <AdminNav currentTab="audit" />
      <AuditURLQuerySearch query={data.query} sudoOnly={data.sudoOnly} />
      <Table
        class="mt-8"
        columns={[
          { title: "Action", class: "w-0", fieldName: "action" },
          { title: "User", class: "w-0", fieldName: "user" },
          { title: "Sudo", class: "w-0" },
          { title: "Meta", class: "w-0" },
          {
            title: "Created",
            class: "w-0",
            fieldName: "created_at",
            align: "right",
          },
        ]}
        pagination={data}
        sortBy={data.sortBy}
        currentUrl={url}
      >
        {data.logs.map((log, idx) => (
          <TableRow key={idx}>
            <TableData>
              {log.action.replaceAll("_", " ")}
            </TableData>
            <TableData>
              <a href={`/user/${log.actor.id}`}>{log.actor.name}</a>
            </TableData>
            <TableData>
              {String(log.isSudo)}
            </TableData>
            <TableData>
              {Object.entries(log.meta).map(([key, value]) => (
                <div>
                  <span>{key}:</span>{" "}
                  <span>
                    {(typeof value === "string" || typeof value === "number")
                      ? value
                      : JSON.stringify(value)}
                  </span>
                </div>
              ))}
            </TableData>
            <TableData
              title={new Date(log.createdAt).toISOString().slice(
                0,
                10,
              )}
              align="right"
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
    const sortBy = ctx.url.searchParams.get("sortBy") || "";
    const page = +(ctx.url.searchParams.get("page") || 1);
    const limit = +(ctx.url.searchParams.get("limit") || 20);
    const sudoOnly = ctx.url.searchParams.get("sudoOnly");

    const resp = await ctx.state.api.get<List<AuditLog>>(
      path`/admin/audit_logs`,
      {
        query,
        sortBy,
        page,
        limit,
        sudoOnly,
      },
    );
    assertOk(resp);

    return {
      data: {
        logs: resp.data.items,
        query,
        sortBy,
        page,
        limit,
        sudoOnly,
        total: resp.data.total,
      },
    };
  },
});
