// Copyright 2024 the JSR authors. All rights reserved. MIT license.
import { HttpError } from "fresh";
import { define } from "../../util.ts";
import { path } from "../../utils/api.ts";
import { FullUser, Scope, User } from "../../utils/api_types.ts";
import { ListPanel } from "../../components/ListPanel.tsx";
import { AccountLayout } from "../account/(_components)/AccountLayout.tsx";

export default define.page<typeof handler>(function UserPage({ data, state }) {
  return (
    <AccountLayout user={data.user} active="Profile">
      <div class="grid grid-cols-1 md:grid-cols-2 gap-6">
        {data.scopes.length > 0
          ? (
            <ListPanel
              title="Scopes"
              subtitle={state.user?.id === data.user.id
                ? "Scopes you are a member of."
                : "Scopes this user belongs to."}
              // deno-lint-ignore jsx-no-children-prop
              children={data.scopes.map((scope) => ({
                value: `@${scope.scope}`,
                href: `/@${scope.scope}`,
              }))}
            />
          )
          : (
            <div class="p-3 text-jsr-gray-500 text-center italic">
              {state.user?.id === data.user.id ? "You are" : "This user is"}
              {" "}
              not a member of any scopes.
            </div>
          )}

        {
          /*<div>
          <span class="font-semibold">Recently published</span>
          <div class="text-jsr-gray-500 text-base">
            TODO: all packages recently published by this user
          </div>
        </div>*/
        }
      </div>
    </AccountLayout>
  );
});

export const handler = define.handlers({
  async GET(ctx) {
    const [currentUser, userRes, scopesRes] = await Promise.all([
      ctx.state.userPromise,
      ctx.state.api.get<User>(path`/users/${ctx.params.id}`),
      ctx.state.api.get<Scope[]>(path`/users/${ctx.params.id}/scopes`),
    ]);
    if (currentUser instanceof Response) return currentUser;

    if (!userRes.ok) {
      if (userRes.code == "userNotFound") {
        throw new HttpError(404, "This user was not found.");
      }

      throw userRes; // gracefully handle errors
    }
    if (!scopesRes.ok) throw scopesRes; // gracefully handle errors

    let user: User | FullUser = userRes.data;
    if (ctx.params.id === currentUser?.id) {
      user = currentUser;
    }

    ctx.state.meta = {
      title: `${user.name} - JSR`,
    };
    return {
      data: {
        user,
        scopes: scopesRes.data,
      },
    };
  },
});
