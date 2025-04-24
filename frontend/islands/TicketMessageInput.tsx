// Copyright 2024 the JSR authors. All rights reserved. MIT license.
import { useState } from "preact/hooks";
import { TbCheck, TbClock } from "tb-icons";
import {
  AdminUpdateTicketRequest,
  FullUser,
  NewTicketMessage,
  Ticket,
} from "../utils/api_types.ts";
import { api, path } from "../utils/api.ts";

export function TicketMessageInput(
  { ticket, user }: { ticket: Ticket; user: FullUser },
) {
  const [message, setMessage] = useState("");

  return (
    <form
      class="space-y-5"
      onSubmit={(e) => {
        e.preventDefault();

        api.post(
          path`/tickets/${ticket.id}`,
          {
            message,
          } satisfies NewTicketMessage,
        ).then((resp) => {
          if (resp.ok) {
            // deno-lint-ignore no-window
            window.location.reload();
          } else {
            console.error(resp);
          }
        });
      }}
    >
      <textarea
        class="w-full block px-2 py-1.5 input-container input min-h-20 bg-white dark:bg-jsr-gray-900"
        value={message}
        rows={3}
        placeholder="Type your message here..."
        onChange={(e) => setMessage(e.currentTarget!.value)}
      />
      <div class="flex justify-end gap-4">
        <button type="submit" class="button-primary">Send message</button>
        {user.isStaff && (
          <button
            type="button"
            class="button-danger"
            onClick={(e) => {
              e.preventDefault();

              api.patch(
                path`/admin/tickets/${ticket.id}`,
                {
                  closed: !ticket.closed,
                } satisfies AdminUpdateTicketRequest,
              ).then((resp) => {
                if (resp.ok) {
                  // deno-lint-ignore no-window
                  window.location.reload();
                } else {
                  console.error(resp);
                }
              });
            }}
          >
            {ticket.closed
              ? (
                <>
                  <TbClock class="text-white" /> Re-open
                </>
              )
              : (
                <>
                  <TbCheck class="text-white" /> Close
                </>
              )} ticket
          </button>
        )}
      </div>
    </form>
  );
}
