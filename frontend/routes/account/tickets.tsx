// Copyright 2024 the JSR authors. All rights reserved. MIT license.
import { HttpError } from "fresh";
import { AccountLayout } from "./(_components)/AccountLayout.tsx";
import { define } from "../../util.ts";
import { Table, TableData, TableRow } from "../../components/Table.tsx";
import { ApiTicket } from "../../utils/api_types.ts";
import { path } from "../../utils/api.ts";
import twas from "twas";
import { TbCheck, TbClock } from "tb-icons";
import { TicketTitle } from "../../components/TicketTitle.tsx";

export default define.page<typeof handler>(function AccountInvitesPage({
  data,
  url,
}) {
  return (
    <AccountLayout user={data.user} active="Tickets">
      <Table
        class="mt-8"
        columns={[
          { title: "Status", class: "w-0" },
          { title: "Kind", class: "w-0" },
          { title: "Created", class: "w-0" },
          { title: "Updated", class: "w-0" },
          { title: "", class: "w-0", align: "right" },
        ]}
        currentUrl={url}
      >
        {data.tickets.map((ticket) => (
          <TableRow key={ticket.id}>
            <TableData>
              <div class="flex items-center gap-1.5">
                {ticket.messages.at(-1)!.author.id !== data.user.id &&
                  !ticket.closed && (
                  <div class="rounded-full bg-orange-600 h-2.5 w-2.5" />
                )}
                <div
                  class={`${
                    ticket.closed ? "bg-green-400" : "bg-orange-400"
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
              <TicketTitle
                kind={ticket.kind}
                meta={ticket.meta}
                user={ticket.creator}
              />
            </TableData>
            <TableData
              title={new Date(ticket.createdAt).toISOString().slice(0, 10)}
            >
              {twas(new Date(ticket.createdAt).getTime())}
            </TableData>
            <TableData
              title={new Date(ticket.updatedAt).toISOString().slice(0, 10)}
            >
              {twas(new Date(ticket.updatedAt).getTime())}
            </TableData>
            <TableData>
              <a class="button-primary" href={`/ticket/${ticket.id}`}>view</a>
            </TableData>
          </TableRow>
        ))}
      </Table>
    </AccountLayout>
  );
});

export const handler = define.handlers({
  async GET(ctx) {
    const [currentUser, ticketsRes] = await Promise.all([
      ctx.state.userPromise,
      ctx.state.api.get<ApiTicket[]>(path`/user/tickets`),
    ]);
    if (currentUser instanceof Response) return currentUser;
    if (!currentUser) throw new HttpError(404, "No signed in user found.");
    if (!ticketsRes.ok) throw ticketsRes; // gracefully handle errors

    ctx.state.meta = { title: "Your tickets - JSR" };
    return {
      data: {
        user: currentUser,
        tickets: ticketsRes.data,
      },
    };
  },
});
