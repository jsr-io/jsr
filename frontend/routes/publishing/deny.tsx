// Copyright 2024 the JSR authors. All rights reserved. MIT license.

import { Handlers, RouteConfig } from "@fresh/core";
import { State } from "../../util.ts";

export default function PublishDeniedPage() {
  return (
    <div class="pb-8 mb-16">
      <h1 class="text-4xl font-bold">Publishing has been denied</h1>
      <p class="text-lg mt-2">
        Go back to the terminal to continue.
      </p>
    </div>
  );
}

export const handler: Handlers<unknown, State> = {
  GET(ctx) {
    ctx.state.meta = {
      title: "Publishing package(s) - JSR",
    };

    return { data: undefined, headers: { "X-Robots-Tag": "noindex" } };
  },
};
