// Copyright 2024 the JSR authors. All rights reserved. MIT license.
// Copyright 2022-2023 the Deno authors. All rights reserved. MIT license.

import { Handlers, PageProps, RouteConfig } from "$fresh/server.ts";
import { State } from "../util.ts";
import { Head } from "$fresh/runtime.ts";

export default function PublishDeniedPage({ data }: PageProps) {
  return (
    <div class="pb-8 mb-16">
      <Head>
        <title>
          Publishing package(s) - JSR
        </title>
      </Head>
      <h1 class="text-4xl font-bold">Publishing has been denied</h1>
      <p class="text-lg mt-2">
        Go back to the terminal to continue.
      </p>
    </div>
  );
}

export const handler: Handlers<State> = {
  GET(req, ctx) {
    return ctx.render(
      undefined,
      { headers: { "X-Robots-Tag": "noindex" } },
    );
  },
};

export const config: RouteConfig = { routeOverride: "/publish-deny" };
