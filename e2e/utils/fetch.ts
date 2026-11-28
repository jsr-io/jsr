// Copyright 2024 the JSR authors. All rights reserved. MIT license.
import { E2E_BYPASS_TOKEN } from "./configuration.ts";

export function jsrFetch(
  input: string,
  init: RequestInit = {},
): Promise<Response> {
  const headers = new Headers(init.headers);
  if (E2E_BYPASS_TOKEN) headers.set("x-jsr-e2e", E2E_BYPASS_TOKEN);
  return fetch(input, { ...init, headers });
}
