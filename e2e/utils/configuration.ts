// Copyright 2024 the JSR authors. All rights reserved. MIT license.
export const JSR_URL: string = Deno.env.get("JSR_URL")!;
export const JSR_API_URL: string = Deno.env.get("JSR_API_URL")!;
export const E2E_BYPASS_TOKEN: string | undefined = Deno.env.get(
  "JSR_E2E_BYPASS_TOKEN",
);

if (!JSR_URL) {
  throw new Error("JSR_URL is not set in the environment");
}

if (!JSR_API_URL) {
  throw new Error("JSR_API_URL is not set in the environment");
}
