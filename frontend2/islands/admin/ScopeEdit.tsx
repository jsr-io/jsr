// Copyright 2024 the JSR authors. All rights reserved. MIT license.
import type { FullScope } from "../../utils/api_types.ts";
import { useState } from "preact/hooks";
import twas from "$twas";
import { api, path } from "../../utils/api.ts";
import { TableData, TableRow } from "../../components/Table.tsx";

export default function AdminScopeEdit({ scope }: { scope: FullScope }) {
  const [edit, setEdit] = useState(false);
  const [processing, setProcessing] = useState(false);
  const [packageLimit, setPackageLimit] = useState(
    String(scope.quotas.packageLimit),
  );
  const [newPackagePerWeekLimit, setNewPackagePerWeekLimit] = useState(
    String(scope.quotas.newPackagePerWeekLimit),
  );
  const [publishAttemptsPerWeekLimit, setPublishAttemptsPerWeekLimit] =
    useState(
      String(scope.quotas.publishAttemptsPerWeekLimit),
    );

  return (
    <TableRow key={scope.scope}>
      <TableData>
        <a href={`/@${scope.scope}`}>{scope.scope}</a>
      </TableData>
      <TableData title={scope.creator.id}>
        <a href={`/user/${scope.creator.id}`}>{scope.creator.name}</a>
      </TableData>
      <TableData>
        {edit
          ? (
            <input
              type="number"
              class="block w-28 p-1.5 text-right input-container input"
              disabled={processing}
              value={packageLimit}
              onChange={(e) => setPackageLimit(e.currentTarget.value)}
              required
            />
          )
          : packageLimit}
      </TableData>
      <TableData>
        {edit
          ? (
            <input
              type="number"
              class="block w-28 p-1.5 text-right input-container input"
              disabled={processing}
              value={newPackagePerWeekLimit}
              onChange={(e) => setNewPackagePerWeekLimit(e.currentTarget.value)}
              required
            />
          )
          : newPackagePerWeekLimit}
      </TableData>
      <TableData>
        {edit
          ? (
            <input
              type="number"
              class="block w-28 p-1.5 text-right input-container input"
              disabled={processing}
              value={publishAttemptsPerWeekLimit}
              onChange={(e) =>
                setPublishAttemptsPerWeekLimit(e.currentTarget.value)}
              required
            />
          )
          : publishAttemptsPerWeekLimit}
      </TableData>
      <TableData title={new Date(scope.createdAt).toISOString().slice(0, 10)}>
        {twas(new Date(scope.createdAt))}
      </TableData>
      <TableData align="right">
        {edit
          ? (
            <>
              <button
                disabled={processing}
                onClick={() => {
                  setProcessing(true);
                  api.patch(path`/admin/scopes/${scope.scope}`, {
                    packageLimit: +packageLimit,
                    newPackagePerWeekLimit: +newPackagePerWeekLimit,
                    versionPublishPerWeekLimit: +publishAttemptsPerWeekLimit,
                  }).then((res) => {
                    setProcessing(false);
                    if (res.ok) {
                      setEdit(false);
                    } else {
                      console.error(res);
                    }
                  });
                }}
                class="link disabled:text-gray-500 disabled:hover:cursor-wait"
              >
                Save<span class="sr-only">, {scope.scope}</span>
              </button>
              <button
                onClick={() => {
                  setEdit(false);
                  setPackageLimit(String(scope.quotas.packageLimit));
                  setNewPackagePerWeekLimit(
                    String(scope.quotas.newPackagePerWeekLimit),
                  );
                  setPublishAttemptsPerWeekLimit(
                    String(scope.quotas.publishAttemptsPerWeekLimit),
                  );
                }}
                class="link ml-2"
              >
                Cancel<span class="sr-only">, {scope.scope}</span>
              </button>
            </>
          )
          : (
            <button
              onClick={() => setEdit(true)}
              class="link"
            >
              Edit<span class="sr-only">, {scope.scope}</span>
            </button>
          )}
      </TableData>
    </TableRow>
  );
}
