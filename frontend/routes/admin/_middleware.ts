// Copyright 2024 the JSR authors. All rights reserved. MIT license.
import { MiddlewareHandler } from "$fresh/server.ts";
import { State } from "../../util.ts";

const isStaff: MiddlewareHandler<State> = async (_req, ctx) => {
  const user = await ctx.state.userPromise;
  if (user instanceof Response) return user;
  if (!user?.isStaff) return ctx.renderNotFound();
  return ctx.next();
};

export const handler: MiddlewareHandler<State>[] = [isStaff];
