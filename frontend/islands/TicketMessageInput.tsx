// Copyright 2024 the JSR authors. All rights reserved. MIT license.
import { useEffect, useState } from "preact/hooks";
import TbCheck from "tb-icons/TbCheck";
import TbClock from "tb-icons/TbClock";
import type {
  AdminUpdateTicketRequest,
  NewTicketMessage,
  TicketStatus,
} from "../utils/api_types.ts";
import { api, path } from "../utils/api.ts";
import { useSignal } from "@preact/signals";

export function TicketMessageInput(
  { ticketId, status, claimToken, isStaff }: {
    ticketId: string;
    status: TicketStatus;
    /// Present when the reporter of an unclaimed ticket got here from the link
    /// in their auto-reply. It stands in for a session, so it is sent with the
    /// reply too.
    claimToken: string | null;
    isStaff: boolean;
  },
) {
  const message = useSignal("");
  const [error, setError] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);

  useEffect(() => {
    if (error) {
      const timeout = setTimeout(() => {
        setError(null);
      }, 3000); // 3 seconds

      return () => clearTimeout(timeout);
    }
  }, [error]);

  const closed = status === "closed";
  const query = claimToken ? { claim: claimToken } : undefined;

  return (
    <form
      class="space-y-5"
      onSubmit={(e) => {
        e.preventDefault();

        if (message.value.trim() === "") {
          setError("Message cannot be empty");
          return;
        }

        setBusy(true);
        api.post(
          path`/tickets/${ticketId}`,
          {
            message: message.value,
          } satisfies NewTicketMessage,
          query,
        ).then((resp) => {
          if (resp.ok) {
            // deno-lint-ignore no-window
            window.location.reload();
          } else {
            console.error(resp);
            setBusy(false);
            setError("Could not send your message. Please try again.");
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
        {
          /* The common status change, as one button. Anything else (spam, the
            waiting-on states) is done through the status control at the top of
            the page, so there is only ever one control per job. */
        }
        {isStaff && (
          <button
            type="button"
            class={closed ? "button-primary" : "button-danger"}
            disabled={busy}
            onClick={() => {
              setBusy(true);
              api.patch(
                path`/admin/tickets/${ticketId}`,
                {
                  status: closed ? "open" : "closed",
                } satisfies AdminUpdateTicketRequest,
              ).then((resp) => {
                if (resp.ok) {
                  // deno-lint-ignore no-window
                  window.location.reload();
                } else {
                  console.error(resp);
                  setBusy(false);
                  setError("Could not update the ticket.");
                }
              });
            }}
          >
            {closed
              ? (
                <>
                  <TbClock /> Reopen ticket
                </>
              )
              : (
                <>
                  <TbCheck /> Close ticket
                </>
              )}
          </button>
        )}
        <button type="submit" class="button-primary" disabled={busy}>
          Send message
        </button>
      </div>
    </form>
  );
}
