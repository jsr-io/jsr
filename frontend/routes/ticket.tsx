// Copyright 2024 the JSR authors. All rights reserved. MIT license.
import { HttpError, RouteConfig } from "fresh";
import { TbArrowLeft, TbCheck, TbClock } from "tb-icons";
import twas from "twas";
import { define } from "../util.ts";
import { path } from "../utils/api.ts";
import { TicketMessageInput } from "../islands/TicketMessageInput.tsx";
import { TicketTitle } from "../components/TicketTitle.tsx";
import type { ApiTicketOverview, TicketKind } from "../utils/api_types.ts";

export default define.page<typeof handler>(function Ticket({
  data,
  state,
}) {
  return (
    <div class="mb-24 space-y-8">
      <div class="flex items-start justify-between gap-6 md:gap-12 max-md:flex-col">
        {state.user!.isStaff && (
          <a class="button-primary" href="/admin/tickets">
            <TbArrowLeft /> Back to admin panel
          </a>
        )}

        <div>
          <p class="text-gray-600 dark:text-gray-300">
            Ticket #{data.ticket.id}
          </p>
          <h1 class="text-3xl font-bold">
            <TicketTitle
              kind={data.ticket.kind}
              meta={data.ticket.meta}
              user={data.ticket.creator}
            />
          </h1>
        </div>

        <div class="flex gap-3 md:gap-8 max-md:flex-col">
          {(formatMeta(data.ticket.kind, data.ticket.meta) ??
            Object.entries(data.ticket.meta)).map((
              [key, value],
            ) => (
              <div key={key}>
                <span class="font-semibold">{key}:</span>
                <br />
                {value}
              </div>
            ))}
          <div>
            <span class="font-semibold">status:</span>
            <br />
            <div class="flex items-center gap-1.5">
              <span>{data.ticket.closed ? "closed" : "open"}</span>
              <div
                class={`${
                  data.ticket.closed
                    ? "bg-green-400 dark:bg-green-600"
                    : "bg-orange-400 dark:bg-orange-600"
                } rounded-full p-1`}
              >
                {data.ticket.closed
                  ? <TbCheck class="text-white" />
                  : <TbClock class="text-white" />}
              </div>
            </div>
          </div>
        </div>
      </div>
      <div class="space-y-3">
        {data.ticket.events.map((event) => {
          if (event.kind === "message") {
            const { message, user } = event;
            const isOpener = user.id === data.ticket.creator.id;

            return (
              <div class="w-full rounded border-1.5 border-current dark:border-cyan-700 px-4 py-3">
                <div class="flex justify-between mb-2">
                  <div class="flex items-center gap-3">
                    <a
                      class="contents"
                      href={`/user/${message.author}`}
                    >
                      <img
                        src={user.avatarUrl}
                        class="w-7 aspect-square rounded-full ring-2 ring-jsr-cyan-700 select-none"
                        alt={user.name}
                      />
                      <span class="font-semibold">{user.name}</span>
                      {" "}
                    </a>
                    <span
                      class={"rounded-full text-sm px-2 inline-block " +
                        (isOpener
                          ? "bg-jsr-cyan-500 text-white"
                          : "bg-jsr-yellow-400 text-jsr-gray-800")}
                    >
                      {isOpener ? "User" : "Staff"}
                    </span>
                  </div>
                  <div>
                    {twas(new Date(message.updatedAt).getTime())}
                  </div>
                </div>
                <pre class="mt-4 font-sans text-wrap">
                {message.message}
                </pre>
              </div>
            );
          } else {
            const { user, auditLog } = event;

            return (
              <div class="flex items-center gap-1.5">
                <div
                  class={`w-fit ${
                    auditLog.meta.closed
                      ? "bg-green-400 dark:bg-green-600"
                      : "bg-orange-400 dark:bg-orange-600"
                  } rounded-full p-1`}
                >
                  {auditLog.meta.closed
                    ? <TbCheck class="text-white" />
                    : <TbClock class="text-white" />}
                </div>
                <p class="text-sm">
                  <span class="font-semibold">{user.name}</span>{" "}
                  {auditLog.meta.closed ? "closed" : "opened"} the ticket{" "}
                  {twas(new Date(auditLog.createdAt).getTime())}
                </p>
              </div>
            );
          }
        })}
      </div>
      {state.user!.id === data.ticket.creator.id &&
        (
          <p class="text-sm text-gray-600 dark:text-gray-300">
            We will respond to you as soon as possible. Please do not create
            multiple tickets for the same issue. You will be emailed at{" "}
            {state.user!.email} when we respond to your ticket.
          </p>
        )}
      <TicketMessageInput
        ticketId={data.ticket.id}
        closed={data.ticket.closed}
        user={state.user!}
      />
    </div>
  );
});

function formatMeta(kind: TicketKind, meta: Record<string, string>) {
  switch (kind) {
    case "user_scope_quota_increase":
      return null;
    case "scope_quota_increase":
      return [[
        "scope",
        // deno-lint-ignore jsx-key
        <a href={`/@${meta.scope}`} class="link">@{meta.scope}</a>,
      ]].concat(Object.entries(meta).filter(([k]) => k !== "scope"));
    case "package_report": {
      const path = `@${meta.scope}/${meta.name}${
        meta.version ? `@${meta.version}` : ""
      }`;
      // deno-lint-ignore jsx-key
      return [["package", <a href={`/${path}`} class="link">{path}</a>]];
    }
    case "scope_claim":
      return null;
    case "other":
      return null;
  }
}

export const handler = define.handlers({
  async GET(ctx) {
    const [currentUser, ticketResp] = await Promise.all([
      ctx.state.userPromise,
      ctx.state.api.get<ApiTicketOverview>(path`/tickets/${ctx.params.ticket}`),
    ]);
    if (currentUser instanceof Response) return currentUser;
    if (!currentUser) throw new HttpError(404, "No signed in user found.");

    if (!ticketResp.ok) throw ticketResp; // gracefully handle this

    ctx.state.meta = {
      title: `Ticket ${ticketResp.data.id} - JSR`,
    };
    return {
      data: {
        ticket: ticketResp.data,
      },
    };
  },
});

export const config: RouteConfig = {
  routeOverride: "/ticket/:ticket",
};
