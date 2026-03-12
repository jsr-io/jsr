// Copyright 2024 the JSR authors. All rights reserved. MIT license.
import { useEffect, useState } from "preact/hooks";
import { TbCheck, TbClock } from "tb-icons";
import {
  AdminUpdateTicketRequest,
  FullUser,
  NewTicketMessage,
} from "../utils/api_types.ts";
import { api, path } from "../utils/api.ts";
import { useSignal } from "@preact/signals";

export function TicketMessageInput(
  { ticketId, closed, user }: {
    ticketId: string;
    closed: boolean;
    user: FullUser;
  },
) {
  const message = useSignal("");
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    if (error) {
      const timeout = setTimeout(() => {
        setError(null);
      }, 3000); // 3 seconds

      return () => clearTimeout(timeout);
    }
  }, [error]);

  return (
    <form
      class="space-y-5"
      onSubmit={(e) => {
        e.preventDefault();

        if (message.value.trim() === "") {
          setError("Message cannot be empty");
          return;
        }

        api.post(
          path`/tickets/${ticketId}`,
          {
            message: message.value,
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
        onChange={(e) => message.value = e.currentTarget!.value}
      />
      <div class="flex justify-end gap-4 items-center">
        {error && (
          <div class="text-red-500 font-semibold">
            <p>
              {error}
            </p>
          </div>
        )}
        <button type="submit" class="button-primary">Send message</button>
        {user.isStaff && (
          <button
            type="button"
            class="button-danger"
            onClick={(e) => {
              e.preventDefault();

              api.patch(
                path`/admin/tickets/${ticketId}`,
                {
                  closed: !closed,
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
            {closed
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
