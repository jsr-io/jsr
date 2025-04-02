// Copyright 2024 the JSR authors. All rights reserved. MIT license.
import { useState } from "preact/hooks";
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
            window.location.reload();
          } else {
            console.error(resp);
          }
        });
      }}
    >
      <textarea
        class="w-full block px-2 py-1.5 input-container input"
        value={message}
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

              console.log("FOO");

              api.patch(
                path`/admin/tickets/${ticket.id}`,
                {
                  closed: !ticket.closed,
                } satisfies AdminUpdateTicketRequest,
              ).then((resp) => {
                if (resp.ok) {
                  window.location.reload();
                } else {
                  console.error(resp);
                }
              });
            }}
          >
            {ticket.closed ? "Reopen" : "Close"} ticket
          </button>
        )}
      </div>
    </form>
  );
}
