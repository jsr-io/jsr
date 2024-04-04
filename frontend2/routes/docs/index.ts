// Copyright 2024 the JSR authors. All rights reserved. MIT license.
import { Handlers } from "$fresh";
import { State } from "../../util.ts";

export const handler: Handlers<undefined, State> = {
  GET() {
    return new Response("", {
      status: 302,
      headers: {
        location: "/docs/introduction",
      },
    });
  },
};
