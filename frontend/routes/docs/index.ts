// Copyright 2024 the JSR authors. All rights reserved. MIT license.
import { define } from "../../util.ts";

export const handler = define.handlers({
  GET() {
    return new Response("", {
      status: 302,
      headers: {
        location: "/docs/introduction",
      },
    });
  },
});
