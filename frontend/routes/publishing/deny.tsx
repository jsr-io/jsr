// Copyright 2024 the JSR authors. All rights reserved. MIT license.

import { Handlers } from "$fresh/server.ts";
import { State } from "../../util.ts";
import { Head } from "$fresh/runtime.ts";

export default function PublishDeniedPage() {
  return (
    <div class="pb-8 mb-16">
      <Head>
        <title>
          Publishing package(s) - JSR
        </title>
        <meta property="og:image" content="/images/og-image.webp" />
      </Head>
      <h1 class="text-4xl font-bold">Publishing has been denied</h1>
      <p class="text-lg mt-2">
        Go back to the terminal to continue.
      </p>
    </div>
  );
}

export const handler: Handlers<unknown, State> = {
  GET(_req, ctx) {
    return ctx.render(
      undefined,
      { headers: { "X-Robots-Tag": "noindex" } },
    );
  },
};
