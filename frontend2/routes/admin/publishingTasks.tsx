// Copyright 2024 the JSR authors. All rights reserved. MIT license.
import { Handlers, PageProps } from "$fresh";
import type { PaginationData, State } from "../../util.ts";
import { Table, TableData, TableRow } from "../../components/Table.tsx";
import { AdminNav } from "./(_components)/AdminNav.tsx";
import { path } from "../../utils/api.ts";
import { List, PublishingTask } from "../../utils/api_types.ts";
import { URLQuerySearch } from "../../components/URLQuerySearch.tsx";
import twas from "$twas";
import PublishingTaskRequeue from "../../islands/PublishingTaskRequeue.tsx";

interface Data extends PaginationData {
  publishingTasks: PublishingTask[];
  query: string;
}

export default function PublishingTasks({ data, url }: PageProps<Data>) {
  return (
    <div class="mb-20">
      <AdminNav currentTab="publishingTasks" />
      <URLQuerySearch query={data.query} />
      <Table
        class="mt-8"
        columns={[
          { title: "ID", class: "w-auto" },
          { title: "Status", class: "w-0" },
          { title: "User ID", class: "w-0" },
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
            <TableData>
              <a href={`/status/${publishingTask.id}`}>{publishingTask.id}</a>
            </TableData>
            <TableData
              title={publishingTask.status === "failure" && publishingTask.error
                ? `Error ${publishingTask.error.code}: ${publishingTask.error.message}`
                : ""}
            >
              {publishingTask.status}
              <br />
              {publishingTask.status === "failure" && publishingTask.error &&
                `Error ${publishingTask.error.code}: ${publishingTask.error.message}`}
            </TableData>
            <TableData>
              <a href={`/user/${publishingTask.userId}`}>
                {publishingTask.userId}
              </a>
            </TableData>
            <TableData>
              <a href={`/@${publishingTask.packageScope}`}>
                {publishingTask.packageScope}
              </a>
            </TableData>
            <TableData>
              <a
                href={`/@${publishingTask.packageScope}/${publishingTask.packageName}`}
              >
                {publishingTask.packageName}
              </a>
            </TableData>
            <TableData>
              <a
                href={`/@${publishingTask.packageScope}/${publishingTask.packageName}/${publishingTask.packageVersion}`}
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
              {twas(new Date(publishingTask.createdAt))}
            </TableData>
            <TableData
              title={new Date(publishingTask.updatedAt).toISOString().slice(
                0,
                10,
              )}
            >
              {twas(new Date(publishingTask.updatedAt))}
            </TableData>
            <TableData>
              <PublishingTaskRequeue publishingTask={publishingTask} />
            </TableData>
          </TableRow>
        ))}
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

    const resp = await ctx.state.api.get<List<PublishingTask>>(
      path`/admin/publishing_tasks`,
      {
        query,
        page,
        limit,
      },
    );
    if (!resp.ok) throw resp; // gracefully handle this

    return ctx.render({
      publishingTasks: resp.data.items,
      query,
      page,
      limit,
      total: resp.data.total,
    });
  },
};
