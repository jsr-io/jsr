// Copyright 2024 the JSR authors. All rights reserved. MIT license.
import { HttpError, RouteConfig } from "fresh";
import { define } from "../util.ts";
import type { Ticket, TicketKind } from "../utils/api_types.ts";
import { path } from "../utils/api.ts";
import twas from "twas";
import { TicketMessageInput } from "../islands/TicketMessageInput.tsx";
import { TbArrowLeft, TbCheck, TbClock } from "tb-icons";
import { TicketTitle } from "../components/TicketTitle.tsx";

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
          <p class="text-gray-600">Ticket #{data.ticket.id}</p>
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
                  data.ticket.closed ? "bg-green-400" : "bg-orange-400"
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
        {data.ticket.messages.map((message) => {
          const isOpener = message.author.id === data.ticket.creator.id;
          return (
            <div class="w-full rounded border-1.5 border-current bg-white px-4 py-3">
              <div class="flex justify-between mb-2">
                <div class="flex items-center gap-3">
                  <a
                    class="contents"
                    href={`/user/${message.author.id}`}
                  >
                    <img
                      src={message.author.avatarUrl}
                      class="w-7 aspect-square rounded-full ring-2 ring-jsr-cyan-700 select-none"
                      alt={message.author.name}
                    />
                    <span class="font-semibold">{message.author.name}</span>
                    {" "}
                  </a>
                  <span
                    class={"rounded-full text-sm px-2 inline-block " +
                      (isOpener
                        ? "bg-jsr-cyan-500 text-white"
                        : "bg-jsr-yellow-400")}
                  >
                    {isOpener ? "User" : "Staff"}
                  </span>
                </div>
                <div>
                  {twas(new Date(message.createdAt).getTime())}
                </div>
              </div>
              <pre class="mt-4 font-sans text-wrap">
                {message.message}
              </pre>
            </div>
          );
        })}
      </div>
      {state.user!.id === data.ticket.creator.id &&
        (
          <p class="text-sm text-gray-600">
            We will respond to you as soon as possible. Please do not create
            multiple tickets for the same issue. You will be emailed at{" "}
            {state.user!.email} when we respond to your ticket.
          </p>
        )}
      <TicketMessageInput ticket={data.ticket} user={state.user!} />
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
      ctx.state.api.get<Ticket>(path`/tickets/${ctx.params.ticket}`),
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
