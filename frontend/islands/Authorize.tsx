// Copyright 2024 the JSR authors. All rights reserved. MIT license.
import { api, path } from "../utils/api.ts";

export default function Authorize(
  props: { code: string; authorizedVersions: string[] },
) {
  const approve = async () => {
    const res = await api.post(
      path`/authorizations/approve/${props.code}`,
      null,
    );

    const url = new URL("/publishing", location.href);
    for (const name of props.authorizedVersions) {
      url.searchParams.append("v", name);
    }
    url.searchParams.set("date", new Date().toISOString());

    if (res.ok) {
      if (props.authorizedVersions.length > 0) {
        globalThis.location.href = url.href;
      } else {
        globalThis.location.href = "/";
      }
    } else {
      console.error(res);
    }
  };

  const deny = async () => {
    const res = await api.post(
      path`/authorizations/deny/${props.code}`,
      null,
    );
    if (res.ok) {
      if (props.authorizedVersions.length > 0) {
        globalThis.location.href = "/publishing/deny";
      } else {
        globalThis.location.href = "/";
      }
    } else {
      console.error(res);
    }
  };

  return (
    <div class="flex gap-2 text-lg mt-4">
      <button
        class="button-primary"
        onClick={approve}
      >
        Approve
      </button>
      <button
        class="button-danger"
        onClick={deny}
      >
        Deny
      </button>
    </div>
  );
}
