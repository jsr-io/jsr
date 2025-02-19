// Copyright 2024 the JSR authors. All rights reserved. MIT license.
import { useState } from "preact/hooks";
import twas from "twas";
import { FullUser } from "../../utils/api_types.ts";
import { api, path } from "../../utils/api.ts";
import { TableData, TableRow } from "../../components/Table.tsx";

export default function UserEdit({ user }: { user: FullUser }) {
  const [edit, setEdit] = useState(false);
  const [processing, setProcessing] = useState(false);
  const [isStaff, setIsStaff] = useState(user.isStaff);
  const [isBlocked, setIsBlocked] = useState(user.isBlocked);
  const [scopeLimit, setScopeLimit] = useState(user.scopeLimit);

  return (
    <TableRow key={user.id}>
      <TableData>
        <a href={`/user/${user.id}`}>{user.name}</a>
      </TableData>
      <TableData>
        {user.email}
      </TableData>
      <TableData>
        {user.githubId}
      </TableData>
      <TableData>
        {edit
          ? (
            <input
              type="number"
              class="block w-16 p-1.5 text-right input-container input"
              disabled={processing}
              value={scopeLimit}
              onChange={(e) => setScopeLimit(+e.currentTarget.value)}
              required
            />
          )
          : scopeLimit}
      </TableData>
      <TableData>
        {edit
          ? (
            <select
              class="block w-16 p-1.5 input-container select"
              onChange={(e) => setIsStaff(e.currentTarget.value === "true")}
              value={String(isStaff)}
              disabled={processing}
            >
              <option value="false">false</option>
              <option value="true">true</option>
            </select>
          )
          : String(isStaff)}
      </TableData>
      <TableData>
        {edit
          ? (
            <select
              class="block w-16 py-2 p-1.5 input-container select"
              onChange={(e) => setIsBlocked(e.currentTarget.value === "true")}
              value={String(isBlocked)}
              disabled={processing}
            >
              <option value="false">false</option>
              <option value="true">true</option>
            </select>
          )
          : String(isBlocked)}
      </TableData>
      <TableData title={new Date(user.createdAt).toISOString().slice(0, 10)}>
        {twas(new Date(user.createdAt).getTime())}
      </TableData>
      <TableData class="relative whitespace-nowrap space-x-3 py-4 pl-3 pr-4 text-right text-sm font-semibold sm:pr-6">
        {edit
          ? (
            <>
              <button
                type="button"
                disabled={processing}
                onClick={() => {
                  setProcessing(true);
                  api.patch(
                    path`/admin/users/${user.id}`,
                    { isStaff, isBlocked, scopeLimit },
                  ).then((res) => {
                    setProcessing(false);
                    if (res.ok) {
                      setEdit(false);
                    } else {
                      console.error(res);
                    }
                  });
                }}
                class="link disabled:text-jsr-gray-500 disabled:cursor-wait"
              >
                Save<span class="sr-only">, {user.name}</span>
              </button>

              <button
                type="button"
                onClick={() => {
                  setEdit(false);
                  setIsStaff(user.isStaff);
                  setIsBlocked(user.isBlocked);
                  setScopeLimit(user.scopeLimit);
                }}
                class="link disabled:text-jsr-gray-500 disabled:cursor-wait"
              >
                Cancel<span class="sr-only">, {user.name}</span>
              </button>
            </>
          )
          : (
            <button
              type="button"
              onClick={() => setEdit(true)}
              class="link disabled:text-jsr-gray-500 disabled:cursor-wait"
            >
              Edit<span class="sr-only">, {user.name}</span>
            </button>
          )}
      </TableData>
    </TableRow>
  );
}
