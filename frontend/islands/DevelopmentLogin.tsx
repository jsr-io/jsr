// Copyright 2024 the JSR authors. All rights reserved. MIT license.
import { useSignal } from "@preact/signals";
import { api } from "../utils/api.ts";
import { path } from "../utils/api.ts";

export function DevelopmentLogin() {
  const waitingForAuth = useSignal(false);

  const onClick = async (e: Event) => {
    e.preventDefault();
    waitingForAuth.value = true;
    const verifier = "abc";
    const challenge = "ungWv48Bz+pBQUDeXa4iI7ADYaOWF3qctBD/YfIAFa0=";

    const res = await api.post(path`/authorizations`, { challenge });
    if (!res.ok) throw new Error("Failed to create authorization");
    // deno-lint-ignore no-explicit-any
    const auth = res.data as any;
    const url = `${auth.verificationUrl}?code=${auth.code}`;
    const w = globalThis.open(url, "_blank");
    while (true) {
      await new Promise((resolve) => setTimeout(resolve, auth.pollInterval));
      // deno-lint-ignore no-explicit-any
      const res = await api.post<any>(path`/authorizations/exchange`, {
        verifier,
        exchangeToken: auth.exchangeToken,
      });
      if (res.ok) {
        waitingForAuth.value = false;
        document.cookie = `token=${res.data.token}; path=/; max-age=31536000`;
        const url = new URL(location.href);
        const redirect = url.searchParams.get("redirect");
        w?.close();
        location.href = redirect ? decodeURIComponent(redirect) : "/";
        return;
      }
      if (res.code !== "authorizationPending") {
        throw new Error("Failed to authenticate: " + JSON.stringify(res));
      }
    }
  };

  return (
    <>
      <button class="button-primary" onClick={onClick}>Authenticate</button>
      {waitingForAuth.value && <p>Waiting for authentication...</p>}
    </>
  );
}
