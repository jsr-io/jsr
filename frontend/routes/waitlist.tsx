// Copyright 2024 the JSR authors. All rights reserved. MIT license.
import { Handlers, PageProps, RouteConfig } from "$fresh/server.ts";
import { FullUser } from "../utils/api_types.ts";
import { State } from "../util.ts";
import { WorksWith } from "../islands/WorksWith.tsx";

export default function Waitlist(props: PageProps<undefined, State>) {
  const user = props.state.user;
  return (
    <div class="w-full bg-gradient-to-b from-slate-950 to-slate-950 via-slate-900 overflow-hidden relative px-4">
      <div class="w-full max-w-screen-md mx-auto min-h-[100dvh] sm:flex flex-col sm:flex-row justify-center items-center gap-x-8">
        <Logo />
        <WorksWith user={user} />
      </div>
    </div>
  );
}

function Logo() {
  return (
    <div class="w-24 sm:w-36 ml-8 mt-16 sm:mt-0 sm:ml-0 h-auto">
      <svg
        xmlns="http://www.w3.org/2000/svg"
        xml:space="preserve"
        style="fill-rule:evenodd;clip-rule:evenodd;stroke-linejoin:round;stroke-miterlimit:2"
        viewBox="0 0 637 343"
      >
        <path
          d="M637.272 49v196h-98v98h-343v-49h-196V98h98V0h343v49h196Z"
          style="fill:#083344"
          transform="translate(-.272)"
        />
        <path
          d="M100.1 196h47.172V49h49v196H51.102v-98H100.1v49Zm488.172-98v98h-49v-49h-49v147h-49V98h147Zm-294 0v49h98v147h-147v-49h98v-49h-98V49h147v49h-98Z"
          style="fill:#f7df1e"
          transform="translate(-.272)"
        />
      </svg>
    </div>
  );
}

export const handler: Handlers<void, State> = {
  async GET(req, ctx) {
    const url = new URL(req.url);
    if (url.searchParams.has("accepted")) {
      const user = await ctx.state.userPromise;
      if (user === null) {
        return new Response("", {
          status: 302,
          headers: { "Location": "/login?redirect=/waitlist" },
        });
      }
      return new Response("", { status: 302, headers: { "Location": "/" } });
    }
    return await ctx.render();
  },
};

export const config: RouteConfig = {
  skipInheritedLayouts: true,
};
