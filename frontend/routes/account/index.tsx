// Copyright 2024 the JSR authors. All rights reserved. MIT license.
import { HttpError } from "fresh";
import { define } from "../../util.ts";

export const handler = define.handlers({
  async GET(ctx) {
    const currentUser = await ctx.state.userPromise;

    if (currentUser instanceof Response) return currentUser;
    if (!currentUser) throw new HttpError(404, "No signed in user found.");

    return new Response("", {
      headers: {
        location: `/user/${currentUser.id}`,
      },
      status: 303,
    });
  },
});
