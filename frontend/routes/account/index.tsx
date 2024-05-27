// Copyright 2024 the JSR authors. All rights reserved. MIT license.
import { Handlers, HttpError } from "@fresh/core";
import { State } from "../../util.ts";

export const handler: Handlers<undefined, State> = {
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
};
