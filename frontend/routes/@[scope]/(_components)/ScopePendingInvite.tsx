// Copyright 2024 the JSR authors. All rights reserved. MIT license.
import { ScopeInvite } from "../../../utils/api_types.ts";

interface ScopePendingInviteProps {
  userInvites: ScopeInvite[] | null;
  scope: string;
}

export function ScopePendingInvite(props: ScopePendingInviteProps) {
  const currentUserPendingInvite = props.userInvites
    ?.find((invite) => invite.scope === props.scope);
  if (!currentUserPendingInvite) return null;

  return (
    <form
      class="mt-8 flex items-center justify-between p-4 bg-jsr-yellow-300 border-jsr-yellow-500 dark:bg-jsr-yellow-900 dark:border-jsr-yellow-700 border rounded-sm flex-row gap-4"
      action={`/@${props.scope}`}
      method="POST"
    >
      <p>
        You have been invited to this scope by{" "}
        {currentUserPendingInvite.requestingUser.name}.
      </p>
      <div class="flex gap-4">
        <button
          type="submit"
          name="action"
          value="reject"
          class="button-danger"
        >
          Reject
        </button>
        <button
          type="submit"
          name="action"
          value="join"
          class="button-primary"
        >
          Join
        </button>
      </div>
    </form>
  );
}
