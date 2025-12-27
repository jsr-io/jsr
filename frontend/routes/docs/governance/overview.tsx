// Copyright 2024 the JSR authors. All rights reserved. MIT license.
import { define } from "../../../util.ts";

/**
 * Redirects to /docs/governance so links to /docs/governance/overview will still work.
 */
export const handler = define.handlers({
  GET() {
    return new Response("", {
      headers: {
        location: `/docs/governance`,
      },
      status: 301,
    });
  },
});
