// Copyright 2024 the JSR authors. All rights reserved. MIT license.
import { define } from "../../util.ts";
import { Table, TableData, TableRow } from "../../components/Table.tsx";
import { AdminNav } from "./(_components)/AdminNav.tsx";
import { path } from "../../utils/api.ts";
import { List, Ticket } from "../../utils/api_types.ts";
import { URLQuerySearch } from "./(_components)/URLQuerySearch.tsx";
import twas from "twas";
import { TbCheck, TbClock } from "tb-icons";

export default define.page<typeof handler>(function Tickets({
  data,
  url,
}) {
  return (
    <div class="mb-20">
      <AdminNav currentTab="tickets" />
      <URLQuerySearch query={data.query} />
      <Table
        class="mt-8"
        columns={[
          { title: "Status", class: "w-0", fieldName: "status" },
          { title: "Creator", class: "w-0", fieldName: "creator" },
          { title: "ID", class: "w-0" },
          { title: "Kind", class: "w-0", fieldName: "kind" },
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
        {data.tickets.map((ticket) => (
          <TableRow key={ticket.id}>
            <TableData>
              <div class="flex items-center gap-1.5">
                {ticket.messages.at(-1)!.author.id === ticket.creator.id &&
                  !ticket.closed && (
                  <div class="rounded-full bg-orange-600 h-2.5 w-2.5" />
                )}
                <div
                  class={`${
                    ticket.closed
                      ? "bg-green-400 dark:bg-green-600"
                      : "bg-orange-400 dark:bg-orange-600"
                  } rounded-full p-1`}
                >
                  {ticket.closed
                    ? <TbCheck class="text-white" />
                    : <TbClock class="text-white" />}
                </div>
                <span>{ticket.closed ? "closed" : "open"}</span>
              </div>
            </TableData>
            <TableData>
              <a
                href={`/admin/users?search=${ticket.creator.id}`}
                class="underline underline-offset-2"
              >
                {ticket.creator.name}
              </a>
            </TableData>
            <TableData>
              <a href={`/ticket/${ticket.id}`}>{ticket.id}</a>
            </TableData>
            <TableData>
              {ticket.kind.replaceAll("_", " ")}
            </TableData>
            <TableData
              title={new Date(ticket.updatedAt).toISOString().slice(
                0,
                10,
              )}
              align="right"
            >
              {twas(new Date(ticket.updatedAt).getTime())}
            </TableData>
            <TableData
              title={new Date(ticket.createdAt).toISOString().slice(
                0,
                10,
              )}
              align="right"
            >
              {twas(new Date(ticket.createdAt).getTime())}
            </TableData>
            <TableData align="right">
              <a class="button-primary" href={`/ticket/${ticket.id}`}>view</a>
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

    const resp = await ctx.state.api.get<List<Ticket>>(
      path`/admin/tickets`,
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
        tickets: resp.data.items,
        query,
        sortBy,
        page,
        limit,
        total: resp.data.total,
      },
    };
  },
});
