// Copyright 2024 the JSR authors. All rights reserved. MIT license.

import { define } from "../util.ts";

export const handler = define.handlers({
  GET(ctx) {
    const redirectPath = ctx.url.searchParams.get("redirect") ?? "/";
    return new Response(null, {
      status: 302,
      headers: {
        "Location": redirectPath,
        "Set-Cookie": `token=; Path=/; Expires=Thu, 01 Jan 1970 00:00:00 GMT`,
      },
    });
  },
});
