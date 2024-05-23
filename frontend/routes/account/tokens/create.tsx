// Copyright 2024 the JSR authors. All rights reserved. MIT license.
import { Handlers, HttpError, PageProps } from "@fresh/core";
import { State } from "../../../util.ts";
import { path } from "../../../utils/api.ts";
import { FullUser, Scope, Token } from "../../../utils/api_types.ts";
import { AccountLayout } from "../(_components)/AccountLayout.tsx";
import twas from "twas";
import { ChevronLeft } from "../../../components/icons/ChevronLeft.tsx";
import { CreateToken } from "./(_islands)/CreateToken.tsx";

interface Data {
  user: FullUser;
}

export default function AccountCreateTokenPage(
  { data, url }: PageProps<Data, State>,
) {
  return (
    <div class="grid grid-cols-1 md:grid-cols-5 gap-4">
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
};
