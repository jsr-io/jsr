// Copyright 2024 the JSR authors. All rights reserved. MIT license.
import { define } from "../../util.ts";
import { Table, TableData, TableRow } from "../../components/Table.tsx";
import { AdminNav } from "./(_components)/AdminNav.tsx";
import { path } from "../../utils/api.ts";
import { List, PublishingTask } from "../../utils/api_types.ts";
import { URLQuerySearch } from "./(_components)/URLQuerySearch.tsx";
import twas from "twas";
import PublishingTaskRequeue from "../../islands/PublishingTaskRequeue.tsx";
import { CopyButton } from "./(_islands)/CopyButton.tsx";

export default define.page<typeof handler>(function PublishingTasks({
  data,
  url,
}) {
  return (
    <div class="mb-20">
      <AdminNav currentTab="publishingTasks" />
      <URLQuerySearch query={data.query} />
      <Table
        class="mt-8"
        columns={[
          { title: "Status", class: "w-0", fieldName: "status" },
          { title: "User", class: "w-0", fieldName: "user" },
          { title: "Package Scope", class: "w-0", fieldName: "scope" },
          { title: "Package Name", class: "w-0", fieldName: "name" },
          { title: "Package Version", class: "w-0", fieldName: "version" },
          {
            title: "Updated",
            class: "w-0",
            fieldName: "updated_at",
            align: "right",
          },
          {
            title: "Created",
            class: "w-0",
            fieldName: "created_at",
            align: "right",
          },
          { title: "", class: "w-0", align: "right" },
        ]}
        pagination={data}
        sortBy={data.sortBy}
        currentUrl={url}
      >
        {data.publishingTasks.map((publishingTask) => (
          <TableRow key={publishingTask.id}>
            <TableData flex>
              <CopyButton value={publishingTask.id} label="copy ID">
                ID
              </CopyButton>
              <div>
                <span class={`font-bold ${publishingTask.status === "failure" ? "text-red-500" : "text-green-500"}`}>
                  {publishingTask.status}
                </span>
                {publishingTask.status === "failure" &&  publishingTask.error && (
                  <span class="font-mono"><br />Error {publishingTask.error.code}: {publishingTask.error.message}</span>
                )}
              </div>
            </TableData>
            <TableData flex>
              {publishingTask.user && (
                <>
                  <CopyButton
                    value={publishingTask.user.id}
                    label="copy user ID"
                  >
                    ID
                  </CopyButton>
                  <a
                    href={`/admin/users?search=${publishingTask.user.id}`}
                    class="underline underline-offset-2"
                  >
                    {publishingTask.user.name}
                  </a>
                </>
              )}
            </TableData>
            <TableData>
              <a
                href={`/admin/scopes?search=${publishingTask.packageScope}`}
                class="underline underline-offset-2"
              >
                {publishingTask.packageScope}
              </a>
            </TableData>
            <TableData>
              <a
                href={`/admin/packages?search=${publishingTask.packageScope}/${publishingTask.packageName}`}
                class="underline underline-offset-2"
              >
                {publishingTask.packageName}
              </a>
            </TableData>
            <TableData>
              <a
                href={`/@${publishingTask.packageScope}/${publishingTask.packageName}/${publishingTask.packageVersion}`}
                class="underline underline-offset-2 font-mono"
              >
                {publishingTask.packageVersion}
              </a>
            </TableData>
            <TableData
              title={new Date(publishingTask.updatedAt).toISOString().slice(
                0,
                10,
              )}
              align="right"
            >
              {twas(new Date(publishingTask.updatedAt).getTime())}
            </TableData>
            <TableData
              title={new Date(publishingTask.createdAt).toISOString().slice(
                0,
                10,
              )}
              align="right"
            >
              {twas(new Date(publishingTask.createdAt).getTime())}
            </TableData>
            <TableData>
              <PublishingTaskRequeue publishingTask={publishingTask} />
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

    const resp = await ctx.state.api.get<List<PublishingTask>>(
      path`/admin/publishing_tasks`,
      {
        query,
        sortBy,
        page,
        limit,
      },
    );
    if (!resp.ok) throw resp; // gracefully handle this

    return {
      data: {
        publishingTasks: resp.data.items,
        query,
        sortBy,
        page,
        limit,
        total: resp.data.total,
      },
    };
  },
});
