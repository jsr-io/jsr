// Copyright 2024 the JSR authors. All rights reserved. MIT license.
import { Handlers, PageProps } from "$fresh/server.ts";
import type { State } from "../../../util.ts";
import { AdminNav } from "../(_components)/AdminNav.tsx";
import { path } from "../../../utils/api.ts";

export default function Scopes({}: PageProps<void, State>) {
  return (
    <div class="mb-20">
      <AdminNav currentTab="scopes" />
      <h2 class="mt-4 text-xl font-sans font-bold">
        Assign scope to user
      </h2>
      <p class="mt-4 max-w-3xl">
        This will assign the given scope to a user, without that user having to
        be invited and accepting the invite. This bypasses the normal reserved
        scope checks, or scope quota limit checks.
      </p>
      <form method="POST" class="flex mt-4">
        <input
          type="text"
          name="scope"
          placeholder="Scope"
          class="block w-full p-1.5 input-container input"
        />
        <input
          type="text"
          name="user_id"
          placeholder="User ID"
          class="ml-4 block w-full p-1.5 input-container input"
        />
        <button type="submit" class="button-primary ml-4">
          Assign
        </button>
      </form>
    </div>
  );
}

export const handler: Handlers<void, State> = {
  async POST(req, ctx) {
    const form = await req.formData();
    const scope = form.get("scope");
    const userId = form.get("user_id");

    const res = await ctx.state.api.post(path`/admin/scopes`, {
      scope,
      userId,
    });
    if (!res.ok) throw res;

    return new Response(null, {
      status: 303,
      headers: { Location: "/admin/scopes" },
    });
  },
};
