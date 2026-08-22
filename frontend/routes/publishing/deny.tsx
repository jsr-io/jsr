// Copyright 2024 the JSR authors. All rights reserved. MIT license.

import { define } from "../../util.ts";

export default define.page<typeof handler>(function PublishDeniedPage() {
  return (
    <div class="pb-8 mb-16">
      <h1 class="text-4xl font-bold">Publishing has been denied</h1>
      <p class="text-lg mt-2">
        Go back to the terminal to continue.
      </p>
    </div>
  );
});

export const handler = define.handlers({
  GET(ctx) {
    ctx.state.meta = {
      title: "Publishing package(s) - JSR",
    };

    return { data: undefined, headers: { "X-Robots-Tag": "noindex" } };
  },
});
