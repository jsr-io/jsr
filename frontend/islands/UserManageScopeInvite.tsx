// Copyright 2024 the JSR authors. All rights reserved. MIT license.
import { TableData } from "../components/Table.tsx";
import { TableRow } from "../components/Table.tsx";
import { api, path } from "../utils/api.ts";
import type { ScopeInvite } from "../utils/api_types.ts";
import { useState } from "preact/hooks";

export default function UserManageScopeInvite(
  { invite }: {
    invite: ScopeInvite;
  },
) {
  const [processing, setProcessing] = useState(false);

  return (
    <TableRow key={invite.scope}>
      <TableData>
        <a
          class="text-cyan-700 hover:text-blue-400 hover:underline"
          href={`/@${invite.scope}`}
        >
          {invite.scope}
        </a>
      </TableData>
      <TableData>
        <a
          class="text-cyan-700 hover:text-blue-400 hover:underline"
          href={`/user/${invite.targetUser.id}`}
        >
          {invite.targetUser.name}
        </a>
      </TableData>
      <TableData>
        <div class="space-x-4">
          <button
            disabled={processing}
            onClick={() => {
              setProcessing(true);
              api.post(path`/user/invites/${invite.scope}`, null).then(
                (res) => {
                  setProcessing(false);
                  if (res.ok) {
                    location.reload();
                  } else {
                    console.error(res);
                  }
                },
              );
            }}
            class="text-indigo-600 hover:text-indigo-900 disabled:text-jsr-gray-500 disabled:cursor-wait"
          >
            Accept invite<span class="sr-only">, {invite.scope}</span>
          </button>
          <button
            disabled={processing}
            onClick={() => {
              setProcessing(true);
              api.delete(path`/user/invites/${invite.scope}`).then((res) => {
                setProcessing(false);
                if (res.ok) {
                  location.reload();
                } else {
                  console.error(res);
                }
              });
            }}
            class="text-red-600 hover:text-red-900 disabled:text-jsr-gray-500 disabled:cursor-wait"
          >
            Decline invite<span class="sr-only">, {invite.scope}</span>
          </button>
        </div>
      </TableData>
    </TableRow>
  );
}
