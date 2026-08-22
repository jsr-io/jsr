// Copyright 2024 the JSR authors. All rights reserved. MIT license.
import { go } from "../../docs/go.ts";
import { define } from "../../util.ts";

export const handler = define.handlers({
  GET({ params }) {
    const id = params.id as string;
    const redirect = go(id) ?? "/docs";
    return new Response(null, {
      status: 302, // Found
      headers: {
        Location: redirect,
      },
    });
  },
});
