// Copyright 2024 the JSR authors. All rights reserved. MIT license.
import { useState } from "preact/hooks";
import { api, path } from "../utils/api.ts";

/// Offered on a ticket that was opened by email and still belongs to an address
/// rather than an account. Claiming binds it to the signed-in account, so it
/// shows up alongside their other tickets instead of only in their inbox.
export function ClaimTicket(
  { ticketId, claimToken, signedIn, returnTo }: {
    ticketId: string;
    claimToken: string;
    signedIn: boolean;
    /// Where to come back to after signing in — this page, claim token and all.
    /// Passed in rather than read from `location`, which does not exist when
    /// this is rendered on the server.
    returnTo: string;
  },
) {
  const [error, setError] = useState<string | null>(null);
  const [claiming, setClaiming] = useState(false);

  return (
    <div class="rounded border-1.5 border-jsr-cyan-700 bg-jsr-cyan-50 dark:bg-jsr-cyan-950 px-4 py-3 space-y-2">
      <p>
        This ticket was opened by email and is not linked to a JSR account yet.
      </p>
      {error && <p class="text-red-500 font-semibold">{error}</p>}
      {signedIn
        ? (
          <button
            type="button"
            class="button-primary"
            disabled={claiming}
            onClick={() => {
              setClaiming(true);
              api.post(
                path`/tickets/${ticketId}/claim`,
                undefined,
                { claim: claimToken },
              ).then((resp) => {
                if (resp.ok) {
                  // Reload without the token: it has been spent, and leaving it
                  // in the URL would only invite it into a bookmark or a paste.
                  // deno-lint-ignore no-window
                  window.location.href = `/ticket/${ticketId}`;
                } else {
                  setClaiming(false);
                  setError(
                    "This claim link is no longer valid. It may already have been used.",
                  );
                }
              });
            }}
          >
            Link this ticket to my account
          </button>
        )
        : (
          <p>
            <a
              class="link"
              href={`/login?redirect=${encodeURIComponent(returnTo)}`}
            >
              Sign in
            </a>{" "}
            to link it to your account. You can also just reply to our email —
            that works either way.
          </p>
        )}
    </div>
  );
}
