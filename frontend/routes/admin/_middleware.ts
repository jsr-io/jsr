// Copyright 2024 the JSR authors. All rights reserved. MIT license.
import { HttpError, Middleware } from "fresh";
import { State } from "../../util.ts";

const isStaff: Middleware<State> = async (ctx) => {
  const user = await ctx.state.userPromise;
  if (user instanceof Response) return user;
  if (!user?.isStaff) throw new HttpError(404, "Not Found");
  return ctx.next();
};

export const handler: Middleware<State>[] = [isStaff];
