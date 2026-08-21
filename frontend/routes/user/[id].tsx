// Copyright 2024 the JSR authors. All rights reserved. MIT license.
import { HttpError } from "fresh";
import { define } from "../../util.ts";
import { assertOk, path } from "../../utils/api.ts";
import { FullUser, Package, Scope, User } from "../../utils/api_types.ts";
import { ScopeCard } from "../../components/ScopeCard.tsx";
import { ListDisplay } from "../../components/List.tsx";
import { PackageHit } from "../../components/PackageHit.tsx";
import { AccountLayout } from "../account/(_components)/AccountLayout.tsx";

export default define.page<typeof handler>(function UserPage({ data, state }) {
  return (
    <AccountLayout user={data.user} active="Profile">
      <div>
        {data.scopes.length > 0
          ? (
            <div>
              <h3 class="text-lg md:text-xl font-semibold">Scopes</h3>
              <p class="text-base text-tertiary mb-4">
                {state.user?.id === data.user.id
                  ? "Scopes you are a member of."
                  : "Scopes this user belongs to."}
              </p>
              <div class="grid grid-cols-1 md:grid-cols-2 gap-3">
                {data.scopes.map((scope) => (
                  <ScopeCard key={scope.scope} scope={scope} />
                ))}
              </div>
            </div>
          )
          : (
            <div class="p-3 text-tertiary text-center italic">
              {state.user?.id === data.user.id ? "You are" : "This user is"}
              {" "}
              not a member of any scopes.
            </div>
          )}

        {data.packages.length > 0 && (
          <div class="mt-8">
            <h3 class="text-lg md:text-xl font-semibold">
              Recently published
            </h3>
            <p class="text-base text-tertiary">
              {state.user?.id === data.user.id
                ? "Packages you have recently published."
                : "Packages this user has recently published."}
            </p>
            <ListDisplay>
              {data.packages.map((entry) => PackageHit(entry))}
            </ListDisplay>
          </div>
        )}
      </div>
    </AccountLayout>
  );
});

export const handler = define.handlers({
  async GET(ctx) {
    const [currentUser, userRes, scopesRes, packagesRes] = await Promise.all([
      ctx.state.userPromise,
      ctx.state.api.get<User>(path`/users/${ctx.params.id}`),
      ctx.state.api.get<Scope[]>(path`/users/${ctx.params.id}/scopes`),
      ctx.state.api.get<Package[]>(path`/users/${ctx.params.id}/packages`),
    ]);
    if (currentUser instanceof Response) return currentUser;

    if (!userRes.ok && userRes.code === "userNotFound") {
      throw new HttpError(404, "This user was not found.");
    }
    assertOk(userRes);
    assertOk(scopesRes);
    assertOk(packagesRes);

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
        packages: packagesRes.data,
      },
    };
  },
});
