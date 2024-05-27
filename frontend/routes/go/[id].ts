// Copyright 2024 the JSR authors. All rights reserved. MIT license.
import { Handlers } from "@fresh/core";
import { go } from "../../docs/go.ts";

export const handler: Handlers = {
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
};
