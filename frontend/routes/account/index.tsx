// Copyright 2024 the JSR authors. All rights reserved. MIT license.
import { Handlers } from "$fresh/server.ts";
import { State } from "../../util.ts";

export const handler: Handlers<undefined, State> = {
  async GET(_, ctx) {
    const currentUser = await ctx.state.userPromise;

    if (currentUser instanceof Response) return currentUser;
    if (!currentUser) return ctx.renderNotFound();

    return new Response("", {
      headers: {
        location: `/user/${currentUser.id}`,
      },
      status: 303,
    });
  },
};
