// Copyright 2024 the JSR authors. All rights reserved. MIT license.
import { api, path } from "../utils/api.ts";

export default function Authorize(
  props: { code: string; packageNames: string[] },
) {
  const approve = async () => {
    const res = await api.post(
      path`/authorizations/approve/${props.code}`,
      null,
    );

    const encodedParams = props.packageNames.length === 1
      ? "packageName=" + encodeURIComponent(props.packageNames[0]) +
        "&noOfPackages=" + encodeURIComponent(1)
      : "packageName=" + encodeURIComponent("") +
        "&noOfPackages=" + encodeURIComponent(props.packageNames.length);

    if (res.ok) {
      window.location.href = "/publish-approve?" + encodedParams;
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
      window.location.href = "/publish-deny";
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
