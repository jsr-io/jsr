// Copyright 2024 the JSR authors. All rights reserved. MIT license.
import { HttpError, RouteConfig } from "fresh";
import TbArrowLeft from "tb-icons/TbArrowLeft";
import TbPaperclip from "tb-icons/TbPaperclip";
import twas from "twas";
import { define } from "../util.ts";
import { assertOk, path } from "../utils/api.ts";
import { TicketMessageInput } from "../islands/TicketMessageInput.tsx";
import { ClaimTicket } from "../islands/ClaimTicket.tsx";
import { TicketActor } from "../components/TicketActor.tsx";
import { TicketStatusControl } from "../islands/TicketStatusControl.tsx";
import {
  TicketStatusDot,
  ticketStatusLabel,
} from "../components/TicketStatus.tsx";
import { TicketTitle } from "../components/TicketTitle.tsx";
import type {
  ApiTicketActor,
  ApiTicketAttachment,
  ApiTicketOverview,
  TicketKind,
  TicketStatus,
} from "../utils/api_types.ts";

export default define.page<typeof handler>(function Ticket({
  data,
  state,
}) {
  const ticket = data.ticket;
  // The reporter of an unclaimed ticket reaches this page with the token from
  // their auto-reply instead of a session, so nothing here may assume a user.
  const isReporter = ticket.reporter.kind === "user"
    ? state.user?.id === ticket.reporter.user.id
    : data.claimToken !== null;

  return (
    <div class="mb-24 space-y-8">
      <div class="flex items-start justify-between gap-6 md:gap-12 max-md:flex-col">
        {state.user?.isStaff && (
          <a class="button-primary" href="/admin/tickets">
            <TbArrowLeft /> Back to admin panel
          </a>
        )}

        <div>
          <p class="text-gray-600 dark:text-gray-300">
            {ticket.ticketNumber}
          </p>
          <h1 class="text-3xl font-bold">
            <TicketTitle
              kind={ticket.kind}
              meta={ticket.meta}
              reporter={ticket.reporter}
              subject={ticket.subject}
            />
          </h1>
        </div>

        <div class="flex gap-3 md:gap-8 max-md:flex-col">
          {(formatMeta(ticket.kind, ticket.meta) ??
            Object.entries(ticket.meta)).map((
              [key, value],
            ) => (
              <div key={key}>
                <span class="font-semibold">{key}:</span>
                <br />
                {value}
              </div>
            ))}
          <TicketStatusControl
            ticketId={ticket.id}
            status={ticket.status}
            canEdit={state.user?.isStaff ?? false}
          />
        </div>
      </div>

      {data.claimToken !== null && ticket.reporter.kind === "email" && (
        <ClaimTicket
          ticketId={ticket.id}
          claimToken={data.claimToken}
          signedIn={state.user !== null}
        />
      )}

      <div class="space-y-3">
        {ticket.events.map((event) => {
          if (event.kind === "message") {
            const { message } = event;

            return (
              <div
                key={message.id}
                class={"w-full rounded border-1.5 px-4 py-3 " +
                  (message.internal
                    // A note is not part of the conversation the reporter can
                    // see, so it should not look like one.
                    ? "border-jsr-yellow-600 bg-jsr-yellow-50 dark:bg-jsr-yellow-950"
                    : "border-current dark:border-cyan-700")}
              >
                <div class="flex justify-between items-start gap-4 mb-2">
                  <div class="flex items-center gap-2 flex-wrap">
                    <TicketActor actor={message.author} />
                    {message.internal
                      ? (
                        <span class="rounded-full text-sm px-2 inline-block bg-jsr-yellow-400 text-jsr-gray-800">
                          Internal note
                        </span>
                      )
                      : (
                        <AuthorRole
                          author={message.author}
                          inbound={message.direction === "inbound"}
                        />
                      )}
                  </div>
                  <div class="text-sm text-gray-600 dark:text-gray-300 shrink-0">
                    {twas(new Date(message.updatedAt).getTime())}
                  </div>
                </div>
                <pre class="mt-4 font-sans text-wrap">
                {message.message}
                </pre>
                {message.attachments.length > 0 && (
                  <Attachments
                    ticketId={ticket.id}
                    claimToken={data.claimToken}
                    attachments={message.attachments}
                  />
                )}
              </div>
            );
          } else {
            return (
              <StatusChange
                key={event.auditLog.createdAt}
                actorName={event.user.name}
                meta={event.auditLog.meta}
                createdAt={event.auditLog.createdAt}
              />
            );
          }
        })}
      </div>

      {isReporter && (
        <p class="text-sm text-gray-600 dark:text-gray-300">
          We will respond to you as soon as possible. Please do not create
          multiple tickets for the same issue. {ticket.reporter.kind === "email"
            ? (
              <>
                You will be emailed at {ticket.reporter.email}{" "}
                when we respond, and you can reply to that email directly.
              </>
            )
            : (
              <>
                You will be emailed at {state.user?.email}{" "}
                when we respond, and you can reply to that email directly.
              </>
            )}
        </p>
      )}

      <TicketMessageInput
        ticketId={ticket.id}
        status={ticket.status}
        claimToken={data.claimToken}
        isStaff={state.user?.isStaff ?? false}
      />
    </div>
  );
});

/// Which side of the conversation a message came from. Derived from the author
/// rather than the direction alone: the automatic acknowledgement is outbound
/// but nobody on the team wrote it, and labelling it "Staff" implies a human
/// replied.
function AuthorRole(
  { author, inbound }: { author: ApiTicketActor; inbound: boolean },
) {
  if (author.kind === "system") {
    return (
      <span class="rounded-full text-sm px-2 inline-block bg-jsr-gray-200 text-jsr-gray-700 dark:bg-jsr-gray-700 dark:text-jsr-gray-100">
        Automatic
      </span>
    );
  }

  return (
    <span
      class={"rounded-full text-sm px-2 inline-block " +
        (inbound
          ? "bg-jsr-cyan-500 text-white"
          : "bg-jsr-yellow-400 text-jsr-gray-800")}
    >
      {inbound ? "User" : "Staff"}
    </span>
  );
}

/// A status change, rendered as a timeline marker rather than a stray line of
/// prose: same left gutter as the message cards, muted, and carrying the colour
/// of the status it moved to.
function StatusChange(
  { actorName, meta, createdAt }: {
    actorName: string;
    meta: Record<string, unknown>;
    createdAt: string;
  },
) {
  // Audit log entries written before the status enum recorded a `closed`
  // boolean instead, and those rows are still in the log.
  const status = (meta.status as TicketStatus | undefined) ??
    (typeof meta.closed === "boolean"
      ? (meta.closed ? "closed" : "open")
      : undefined);

  return (
    <div class="flex items-center gap-2 pl-4 text-sm text-gray-600 dark:text-gray-300">
      {status
        ? <TicketStatusDot status={status} />
        : <div class="rounded-full bg-jsr-gray-400 p-1 shrink-0" />}
      <p>
        <span class="font-semibold">{actorName}</span> set the status to{" "}
        <span class="font-semibold">
          {status ? ticketStatusLabel(status) : "a new value"}
        </span>{" "}
        · {twas(new Date(createdAt).getTime())}
      </p>
    </div>
  );
}

function Attachments(
  { ticketId, claimToken, attachments }: {
    ticketId: string;
    claimToken: string | null;
    attachments: ApiTicketAttachment[];
  },
) {
  const query = claimToken ? `?claim=${encodeURIComponent(claimToken)}` : "";
  return (
    <ul class="mt-4 space-y-1">
      {attachments.map((attachment) => (
        <li key={attachment.id}>
          <a
            class="link inline-flex items-center gap-1.5 text-sm"
            href={`/api/tickets/${ticketId}/attachments/${attachment.id}${query}`}
          >
            <TbPaperclip />
            {attachment.filename}
            <span class="text-gray-600 dark:text-gray-300">
              ({formatBytes(attachment.sizeBytes)})
            </span>
          </a>
        </li>
      ))}
    </ul>
  );
}

function formatBytes(bytes: number): string {
  if (bytes < 1024) return `${bytes} B`;
  if (bytes < 1024 * 1024) return `${(bytes / 1024).toFixed(1)} KB`;
  return `${(bytes / (1024 * 1024)).toFixed(1)} MB`;
}

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
    // Present when the visitor followed the claim link out of the auto-reply
    // email. It stands in for a session, so it is forwarded to the API.
    const claimToken = ctx.url.searchParams.get("claim");

    const [currentUser, ticketResp] = await Promise.all([
      ctx.state.userPromise,
      ctx.state.api.get<ApiTicketOverview>(
        path`/tickets/${ctx.params.ticket}`,
        claimToken ? { claim: claimToken } : undefined,
      ),
    ]);
    if (currentUser instanceof Response) return currentUser;
    if (!currentUser && !claimToken) {
      throw new HttpError(404, "No signed in user found.");
    }

    assertOk(ticketResp);

    ctx.state.meta = {
      title: `Ticket ${ticketResp.data.ticketNumber} - JSR`,
    };
    return {
      data: {
        ticket: ticketResp.data,
        claimToken,
      },
    };
  },
});

export const config: RouteConfig = {
  routeOverride: "/ticket/:ticket",
};
