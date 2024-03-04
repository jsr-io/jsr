// Copyright 2024 the JSR authors. All rights reserved. MIT license.
import { Handlers } from "$fresh/server.ts";
import { State } from "../util.ts";

export const handler: Handlers<void, State> = {
  GET() {
    return new Response("", {
      status: 302,
      headers: { "Location": "/" },
    });
  },
};
