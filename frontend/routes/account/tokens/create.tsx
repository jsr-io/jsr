// Copyright 2024 the JSR authors. All rights reserved. MIT license.
import { Handlers } from "$fresh/server.ts";
import { State } from "../../../util.ts";
import { FullUser } from "../../../utils/api_types.ts";
import { Head } from "$fresh/runtime.ts";
import { ChevronLeft } from "../../../components/icons/ChevronLeft.tsx";
import { CreateToken } from "./(_islands)/CreateToken.tsx";

interface Data {
  user: FullUser;
}

export default function AccountCreateTokenPage() {
  return (
    <div class="grid grid-cols-1 md:grid-cols-5 gap-4">
      <Head>
        <title>
          Create a personal access token - JSR
        </title>
      </Head>
      <div>
        <a href="/account/tokens" class="link flex items-center gap-2">
          <ChevronLeft class="w-6 h-6" />
          <span class="ml-2">Back to tokens</span>
        </a>
      </div>
      <div class="col-span-1 md:col-span-4">
        <h2 class="text-xl font-bold">
          Create a personal access token
        </h2>
        <p class="text-gray-600 max-w-2xl mt-2">
          Personal access tokens can be used to authenticate with JSR from the
          command line or from other applications.
        </p>
        <p class="text-gray-600 max-w-2xl mt-3">
          Actions performed by personal access tokens are attributed to your
          account.
        </p>
        <CreateToken />
      </div>
    </div>
  );
}

export const handler: Handlers<Data, State> = {
  async GET(_, ctx) {
    const currentUser = await ctx.state.userPromise;
    if (currentUser instanceof Response) return currentUser;
    if (!currentUser) return ctx.renderNotFound();

    return ctx.render({
      user: currentUser,
    });
  },
};
