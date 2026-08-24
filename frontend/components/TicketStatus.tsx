// Copyright 2024 the JSR authors. All rights reserved. MIT license.
import type { JSX } from "preact";
import TbAlertTriangle from "tb-icons/TbAlertTriangle";
import TbCheck from "tb-icons/TbCheck";
import TbClock from "tb-icons/TbClock";
import TbMessage from "tb-icons/TbMessage";
import type { TicketStatus } from "../utils/api_types.ts";

/// The order staff work tickets in, and the order the status control offers
/// them.
export const TICKET_STATUSES: TicketStatus[] = [
  "open",
  "waiting_on_support",
  "waiting_on_user",
  "closed",
  "spam",
];

export function ticketStatusLabel(status: TicketStatus): string {
  switch (status) {
    case "open":
      return "open";
    case "waiting_on_support":
      return "waiting on us";
    case "waiting_on_user":
      return "waiting on you";
    case "closed":
      return "closed";
    case "spam":
      return "spam";
  }
}

/// Whether the ticket is still being worked. `spam` counts as resolved: it is a
/// parking spot for junk mail, not part of the queue.
export function isTicketActive(status: TicketStatus): boolean {
  return status !== "closed" && status !== "spam";
}

function statusStyle(
  status: TicketStatus,
): { color: string; icon: JSX.Element } {
  switch (status) {
    case "open":
      return {
        color: "bg-orange-400 dark:bg-orange-600",
        icon: <TbClock class="text-white" />,
      };
    case "waiting_on_support":
      return {
        color: "bg-orange-400 dark:bg-orange-600",
        icon: <TbMessage class="text-white" />,
      };
    case "waiting_on_user":
      return {
        color: "bg-blue-400 dark:bg-blue-600",
        icon: <TbMessage class="text-white" />,
      };
    case "closed":
      return {
        color: "bg-green-400 dark:bg-green-600",
        icon: <TbCheck class="text-white" />,
      };
    case "spam":
      return {
        color: "bg-jsr-gray-400 dark:bg-jsr-gray-600",
        icon: <TbAlertTriangle class="text-white" />,
      };
  }
}

/// Just the coloured marker, without the label. Used in the timeline, where the
/// surrounding sentence already names the status.
export function TicketStatusDot(
  { status }: { status: TicketStatus },
): JSX.Element {
  const { color, icon } = statusStyle(status);
  return (
    <div class={`${color} rounded-full p-1 shrink-0`}>
      {icon}
    </div>
  );
}

export function TicketStatusBadge(
  { status, unread }: { status: TicketStatus; unread?: boolean },
): JSX.Element {
  return (
    <div class="flex items-center gap-1.5">
      {unread
        ? <div class="rounded-full bg-orange-600 h-2.5 w-2.5" />
        // Keeps the badge aligned with rows that do carry an unread dot.
        : <div class="h-2.5 w-2.5" />}
      <TicketStatusDot status={status} />
      <span>{ticketStatusLabel(status)}</span>
    </div>
  );
}
