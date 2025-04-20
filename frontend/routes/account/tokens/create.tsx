// Copyright 2024 the JSR authors. All rights reserved. MIT license.
import { HttpError } from "fresh";
import { define } from "../../../util.ts";
import TbChevronLeft from "tb-icons/TbChevronLeft";
import { CreateToken } from "./(_islands)/CreateToken.tsx";

export default define.page<typeof handler>(function AccountCreateTokenPage() {
  return (
    <div class="grid grid-cols-1 md:grid-cols-5 gap-4">
      <div>
        <a href="/account/tokens" class="link flex items-center gap-2">
          <TbChevronLeft class="size-8" />
          <span class="ml-2">Back to tokens</span>
        </a>
      </div>
      <div class="col-span-1 md:col-span-4">
        <h2 class="text-xl font-bold">
          Create a personal access token
        </h2>
        <p class="text-secondary max-w-2xl mt-2">
          Personal access tokens can be used to authenticate with JSR from the
          command line or from other applications.
        </p>
        <p class="text-secondary max-w-2xl mt-3">
          Actions performed by personal access tokens are attributed to your
          account.
        </p>
        <CreateToken />
      </div>
    </div>
  );
});

export const handler = define.handlers({
  async GET(ctx) {
    const currentUser = await ctx.state.userPromise;
    if (currentUser instanceof Response) return currentUser;
    if (!currentUser) throw new HttpError(404, "No signed in user found.");

    ctx.state.meta = { title: "Create personal access token - JSR" };
    return {
      data: {
        user: currentUser,
      },
    };
  },
});
