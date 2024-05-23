// Copyright 2024 the JSR authors. All rights reserved. MIT license.
import { Handlers } from "@fresh/core";
import { State } from "../../../util.ts";

export const handler: Handlers<undefined, State> = {
  GET(ctx) {
    return new Response("", {
      status: 302,
      headers: {
        Location: `/@${ctx.params.scope}`,
      },
    });
  },
};
