// Copyright 2024 the JSR authors. All rights reserved. MIT license.
//
// This file replaces `og.ts` in the Cloud Run docker image. The real
// `og.ts` imports `workers-og`, which has static `.wasm` imports that
// Deno's module-graph analyzer chokes on at startup (it tries to walk
// the wasm module's internal import section as JS). Cloud Run is only
// a fallback for the new Cloudflare Worker frontend, and the LB routes
// all `/og` requests to the CF Worker — so this handler never runs in
// production. It exists only to keep Fresh's route discovery happy.
import { HttpError, RouteConfig } from "fresh";
import { define } from "../../util.ts";

export const handler = define.handlers({
  GET() {
    throw new HttpError(
      503,
      "OG image generation is only served by the Cloudflare Worker frontend",
    );
  },
});

export const config: RouteConfig = {
  routeOverride: "/@:scope/:package{@:version}?/og",
};
