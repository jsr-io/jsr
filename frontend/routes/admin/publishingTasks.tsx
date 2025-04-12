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
          { title: "Status", class: "w-0" },
          { title: "User", class: "w-0" },
          { title: "Package Scope", class: "w-0" },
          { title: "Package Name", class: "w-0" },
          { title: "Package Version", class: "w-0" },
          { title: "Created", class: "w-0" },
          { title: "Updated", class: "w-0" },
          { title: "", class: "w-0", align: "right" },
        ]}
        pagination={data}
        currentUrl={url}
      >
        {data.publishingTasks.map((publishingTask) => (
          <TableRow key={publishingTask.id}>
            <TableData flex>
              <CopyButton value={publishingTask.id} label="copy ID">
                ID
              </CopyButton>
              <div>
                {publishingTask.status}
                <br />
                {publishingTask.status === "failure" && publishingTask.error &&
                  `Error ${publishingTask.error.code}: ${publishingTask.error.message}`}
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
                class="underline underline-offset-2"
              >
                {publishingTask.packageVersion}
              </a>
            </TableData>
            <TableData
              title={new Date(publishingTask.createdAt).toISOString().slice(
                0,
                10,
              )}
            >
              {twas(new Date(publishingTask.createdAt).getTime())}
            </TableData>
            <TableData
              title={new Date(publishingTask.updatedAt).toISOString().slice(
                0,
                10,
              )}
            >
              {twas(new Date(publishingTask.updatedAt).getTime())}
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
    const page = +(ctx.url.searchParams.get("page") || 1);
    const limit = +(ctx.url.searchParams.get("limit") || 20);

    const resp = await ctx.state.api.get<List<PublishingTask>>(
      path`/admin/publishing_tasks`,
      {
        query,
        page,
        limit,
      },
    );
    if (!resp.ok) throw resp; // gracefully handle this

    return {
      data: {
        publishingTasks: resp.data.items,
        query,
        page,
        limit,
        total: resp.data.total,
      },
    };
  },
});
