// Copyright 2024 the JSR authors. All rights reserved. MIT license.
import { define } from "../../util.ts";

export const handler = define.handlers({
  GET() {
    return new Response(null, {
      headers: { location: "/admin/scopes" },
      status: 307,
    });
  },
});
