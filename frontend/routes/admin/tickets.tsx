// Copyright 2024 the JSR authors. All rights reserved. MIT license.
import { define } from "../../util.ts";
import { Table, TableData, TableRow } from "../../components/Table.tsx";
import { AdminNav } from "./(_components)/AdminNav.tsx";
import { assertOk, path } from "../../utils/api.ts";
import type { ApiTicket, List, TicketStatus } from "../../utils/api_types.ts";
import { URLQuerySearch } from "./(_components)/URLQuerySearch.tsx";
import twas from "twas";
import {
  isTicketActive,
  TICKET_STATUSES,
  TicketStatusBadge,
  ticketStatusLabel,
} from "../../components/TicketStatus.tsx";
import { TicketTitle } from "../../components/TicketTitle.tsx";
import { ticketActorName } from "../../components/TicketActor.tsx";

export default define.page<typeof handler>(function Tickets({
  data,
  url,
}) {
  return (
    <div class="mb-20">
      <AdminNav currentTab="tickets" />
      <URLQuerySearch query={data.query} />
      <StatusFilter current={data.status} url={url} />
      <Table
        class="mt-8"
        columns={[
          { title: "Status", class: "w-0", fieldName: "status" },
          { title: "Reporter", class: "w-0", fieldName: "creator" },
          { title: "Ticket", class: "w-0" },
          { title: "Subject", fieldName: "kind" },
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
        {data.tickets.map((ticket) => {
          // The last word came from the reporter, so this ticket is waiting on
          // staff to answer it.
          const isNotification = isTicketActive(ticket.status) &&
            ticket.messages.at(-1)!.direction === "inbound";

          return (
            <TableRow key={ticket.id}>
              <TableData>
                <TicketStatusBadge
                  status={ticket.status}
                  unread={isNotification}
                />
              </TableData>
              <TableData>
                {ticket.reporter.kind === "user"
                  ? (
                    <a
                      href={`/admin/users?search=${ticket.reporter.user.id}`}
                      class="underline underline-offset-2"
                    >
                      {ticket.reporter.user.name}
                    </a>
                  )
                  // An email-opened ticket that nobody has claimed: there is no
                  // account to link to, only the address it came from.
                  : (
                    <span title={ticketActorName(ticket.reporter)}>
                      {ticketActorName(ticket.reporter)}
                    </span>
                  )}
              </TableData>
              <TableData>
                <a href={`/ticket/${ticket.id}`}>{ticket.ticketNumber}</a>
              </TableData>
              <TableData>
                <TicketTitle
                  kind={ticket.kind}
                  meta={ticket.meta}
                  reporter={ticket.reporter}
                  subject={ticket.subject}
                />
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
          );
        })}
      </Table>
    </div>
  );
});

export const handler = define.handlers({
  async GET(ctx) {
    const query = ctx.url.searchParams.get("search") || "";
    const sortBy = ctx.url.searchParams.get("sortBy") || "";
    const status = ctx.url.searchParams.get("status") || "";
    const page = +(ctx.url.searchParams.get("page") || 1);
    const limit = +(ctx.url.searchParams.get("limit") || 20);

    const resp = await ctx.state.api.get<List<ApiTicket>>(
      path`/admin/tickets`,
      {
        query,
        sortBy,
        status,
        page,
        limit,
      },
    );
    assertOk(resp);

    return {
      data: {
        tickets: resp.data.items,
        query,
        sortBy,
        status: status as TicketStatus | "",
        page,
        limit,
        total: resp.data.total,
      },
    };
  },
});

/// Status filter, rendered as links so it needs no client-side JavaScript. Each
/// link keeps the rest of the query string and resets to the first page.
function StatusFilter(
  { current, url }: { current: TicketStatus | ""; url: URL },
) {
  function href(status: TicketStatus | "") {
    const next = new URL(url);
    if (status) {
      next.searchParams.set("status", status);
    } else {
      next.searchParams.delete("status");
    }
    next.searchParams.delete("page");
    return next.pathname + next.search;
  }

  const options: (TicketStatus | "")[] = ["", ...TICKET_STATUSES];

  return (
    <div class="mt-4 flex flex-wrap gap-2 items-center">
      <span class="text-sm text-gray-600 dark:text-gray-300">status:</span>
      {options.map((status) => (
        <a
          key={status || "all"}
          href={href(status)}
          class={"rounded-full text-sm px-3 py-0.5 " +
            (status === current
              ? "bg-jsr-cyan-500 text-white"
              : "bg-jsr-gray-100 dark:bg-jsr-gray-800 hover:bg-jsr-gray-200 dark:hover:bg-jsr-gray-700")}
        >
          {status ? ticketStatusLabel(status) : "all"}
        </a>
      ))}
    </div>
  );
}
