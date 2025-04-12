// Copyright 2024 the JSR authors. All rights reserved. MIT license.
import type { FullScope, List } from "../../../utils/api_types.ts";
import { Table, TableData, TableRow } from "../../../components/Table.tsx";
import { path } from "../../../utils/api.ts";
import { AdminNav } from "../(_components)/AdminNav.tsx";
import { URLQuerySearch } from "../(_components)/URLQuerySearch.tsx";
import { define } from "../../../util.ts";
import TbArrowRight from "tb-icons/TbArrowRight";
import twas from "twas";
import { CopyButton } from "../(_islands)/CopyButton.tsx";
import { EditModal } from "../(_islands)/EditModal.tsx";

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
          { title: "Name", class: "w-0", fieldName: "scope" },
          { title: "Creator", class: "w-0", fieldName: "creator" },
          { title: "Package Limit", class: "w-0", fieldName: "package_limit" },
          {
            title: "Package per Week Limit",
            class: "w-0",
            fieldName: "new_package_per_week_limit",
          },
          {
            title: "Publishes per Week Limit",
            class: "w-0",
            fieldName: "publish_attempts_per_week_limit",
          },
          {
            title: "Created",
            class: "w-0",
            align: "right",
            fieldName: "created_at",
          },
          { title: "", class: "w-0" },
        ]}
        pagination={data}
        sortBy={data.sortBy}
        currentUrl={url}
      >
        {data.scopes.map((scope) => (
          <TableRow key={scope.scope}>
            <TableData>
              <a href={`/@${scope.scope}`} class="underline underline-offset-2">
                {scope.scope}
              </a>
            </TableData>
            <TableData flex>
              <CopyButton value={scope.creator.id} label="copy user ID">
                ID
              </CopyButton>
              <a
                href={`/admin/users?search=${scope.creator.id}`}
                class="underline underline-offset-2"
              >
                {scope.creator.name}
              </a>
            </TableData>
            <TableData>
              {scope.quotas.packageLimit}
            </TableData>
            <TableData>
              {scope.quotas.newPackagePerWeekLimit}
            </TableData>
            <TableData>
              {scope.quotas.publishAttemptsPerWeekLimit}
            </TableData>
            <TableData
              title={new Date(scope.createdAt).toISOString().slice(0, 10)}
              align="right"
            >
              {twas(new Date(scope.createdAt).getTime())}
            </TableData>
            <TableData>
              <EditModal
                style="primary"
                path={path`/admin/scopes/${scope.scope}`}
                title={`Edit scope '${scope.scope}'`}
                fields={[
                  {
                    name: "packageLimit",
                    label: "Package Limit",
                    type: "number",
                    value: scope.quotas.packageLimit,
                  },
                  {
                    name: "newPackagePerWeekLimit",
                    label: "Package per Week Limit",
                    type: "number",
                    value: scope.quotas.newPackagePerWeekLimit,
                  },
                  {
                    name: "publishAttemptsPerWeekLimit",
                    label: "Publishes per Week Limit",
                    type: "number",
                    value: scope.quotas.publishAttemptsPerWeekLimit,
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
    const sortBy = ctx.url.searchParams.get("sortBy") || "created_at";
    const page = +(ctx.url.searchParams.get("page") || 1);
    const limit = +(ctx.url.searchParams.get("limit") || 20);

    const resp = await ctx.state.api.get<List<FullScope>>(path`/admin/scopes`, {
      query,
      page,
      limit,
      sortBy,
    });
    if (!resp.ok) throw resp; // gracefully handle this

    return {
      data: {
        scopes: resp.data.items,
        query,
        sortBy,
        page,
        limit,
        total: resp.data.total,
      },
    };
  },
});
