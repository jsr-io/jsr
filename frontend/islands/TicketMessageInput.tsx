// Copyright 2024 the JSR authors. All rights reserved. MIT license.
import { useEffect, useState } from "preact/hooks";
import TbCheck from "tb-icons/TbCheck";
import TbClock from "tb-icons/TbClock";
import type {
  AdminUpdateTicketRequest,
  NewTicketMessage,
  TicketStatus,
} from "../utils/api_types.ts";
import {
  remainingPlaceholders,
  TICKET_TEMPLATES,
} from "../utils/ticket_templates.ts";
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
  /// Writing a note to the rest of the team rather than replying to the person
  /// who opened the ticket.
  const [internal, setInternal] = useState(false);
  const [template, setTemplate] = useState("");

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
  // Only worth pointing out on a reply; a note is for the team and can say
  // whatever it likes.
  const placeholders = internal ? [] : remainingPlaceholders(message.value);

  const categories = [...new Set(TICKET_TEMPLATES.map((t) => t.category))];

  return (
    <form
      class="space-y-3"
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
            internal,
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
      {isStaff && (
        <div class="flex flex-wrap items-center gap-3">
          <label class="flex items-center gap-2 text-sm select-none dark:text-gray-200">
            <input
              type="checkbox"
              class="dark:bg-jsr-gray-900 dark:border-gray-700"
              checked={internal}
              onChange={(e) => setInternal(e.currentTarget.checked)}
            />
            <span>Internal note (only staff can see this)</span>
          </label>

          <div class="flex items-center gap-2 ml-auto">
            <label class="text-sm" for="ticket-template">Template:</label>
            <select
              id="ticket-template"
              class="input-container select py-1"
              value={template}
              onChange={(e) => setTemplate(e.currentTarget.value)}
            >
              <option value="">Choose one…</option>
              {categories.map((category) => (
                <optgroup key={category} label={category}>
                  {TICKET_TEMPLATES.filter((t) =>
                    t.category === category
                  ).map(
                    (t) => <option key={t.id} value={t.id}>{t.label}</option>,
                  )}
                </optgroup>
              ))}
            </select>
            <button
              type="button"
              class="button-primary py-1"
              disabled={template === ""}
              onClick={() => {
                const chosen = TICKET_TEMPLATES.find((t) => t.id === template);
                if (!chosen) return;
                // Appended rather than substituted, so a draft already being
                // written is never thrown away by picking a template.
                const existing = message.value.trimEnd();
                message.value = existing === ""
                  ? chosen.body
                  : `${existing}\n\n${chosen.body}`;
                setTemplate("");
              }}
            >
              Insert
            </button>
          </div>
        </div>
      )}

      <textarea
        class={"w-full block px-2 py-1.5 input-container input min-h-20 " +
          (internal
            ? "bg-jsr-yellow-50 dark:bg-jsr-yellow-950"
            : "bg-white dark:bg-jsr-gray-900")}
        value={message}
        rows={3}
        placeholder={internal
          ? "Write a note for other staff…"
          : "Type your message here..."}
        onChange={(e) => message.value = e.currentTarget!.value}
      />

      {placeholders.length > 0 && (
        // Sending "I've increased your scope quota to [new_limit]" verbatim is
        // the obvious way to misuse a template, so it is called out. Not
        // blocking: square brackets are legitimate prose, and a draft that
        // cannot be sent is worse than one that is flagged.
        <p class="text-sm text-jsr-yellow-800 dark:text-jsr-yellow-200">
          Still to fill in: {placeholders.join(", ")}
        </p>
      )}

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
          {internal ? "Add note" : "Send message"}
        </button>
      </div>
    </form>
  );
}
