// Copyright 2024 the JSR authors. All rights reserved. MIT license.
import { Handlers } from "$fresh/server.ts";

export const handler: Handlers = {
  GET(_req, ctx) {
    const redirectPath = ctx.url.searchParams.get("redirect") ?? "/";
    return new Response(null, {
      status: 302,
      headers: {
        "Location": redirectPath,
        "Set-Cookie": `token=; Path=/; Expires=Thu, 01 Jan 1970 00:00:00 GMT`,
      },
    });
  },
};
