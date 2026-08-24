// Copyright 2024 the JSR authors. All rights reserved. MIT license.
import { useState } from "preact/hooks";
import type {
  AdminUpdateTicketRequest,
  TicketStatus,
} from "../utils/api_types.ts";
import {
  TICKET_STATUSES,
  TicketStatusBadge,
  ticketStatusLabel,
} from "../components/TicketStatus.tsx";
import { api, path } from "../utils/api.ts";

/// The ticket's status, and — for staff — the control that changes it.
///
/// Picking a value does not apply it. A native select fires `change` while the
/// keyboard moves through its options, so applying on change means arrowing past
/// "spam" silently marks the ticket as spam. The change only happens when the
/// separate button is pressed, which also gives the reader somewhere to see what
/// they are about to do.
export function TicketStatusControl(
  { ticketId, status, canEdit }: {
    ticketId: string;
    status: TicketStatus;
    canEdit: boolean;
  },
) {
  const [selected, setSelected] = useState<TicketStatus>(status);
  const [saving, setSaving] = useState(false);
  const [error, setError] = useState<string | null>(null);

  if (!canEdit) {
    return (
      <div>
        <span class="font-semibold">status:</span>
        <br />
        <TicketStatusBadge status={status} />
      </div>
    );
  }

  const dirty = selected !== status;

  return (
    <div>
      <label
        class="font-semibold"
        for="ticket-status"
      >
        status:
      </label>
      <div class="flex items-center gap-2 mt-1">
        <select
          id="ticket-status"
          class="input-container select py-1"
          value={selected}
          disabled={saving}
          onChange={(e) => setSelected(e.currentTarget.value as TicketStatus)}
        >
          {TICKET_STATUSES.map((option) => (
            <option key={option} value={option}>
              {ticketStatusLabel(option)}
            </option>
          ))}
        </select>
        <button
          type="button"
          class="button-primary py-1"
          // Nothing to apply until the selection actually differs, which also
          // makes it obvious that picking a value on its own changed nothing.
          disabled={!dirty || saving}
          onClick={() => {
            setSaving(true);
            setError(null);
            api.patch(
              path`/admin/tickets/${ticketId}`,
              { status: selected } satisfies AdminUpdateTicketRequest,
            ).then((resp) => {
              if (resp.ok) {
                // deno-lint-ignore no-window
                window.location.reload();
              } else {
                setSaving(false);
                setError("Could not update the status.");
              }
            });
          }}
        >
          {saving ? "Updating…" : "Update"}
        </button>
      </div>
      {error && <p class="text-red-500 text-sm mt-1">{error}</p>}
    </div>
  );
}
