// Copyright 2024 the JSR authors. All rights reserved. MIT license.
import { api, path } from "../utils/api.ts";

export default function Authorize(props: { code: string }) {
  const approve = async () => {
    const res = await api.post(
      path`/authorizations/approve/${props.code}`,
      null,
    );
    if (res.ok) {
      // TODO: redirect to somewhere more useful
      window.location.href = "/";
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
      // TODO: redirect to somewhere more useful
      window.location.href = "/";
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
