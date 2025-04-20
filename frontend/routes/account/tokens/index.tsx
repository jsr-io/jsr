// Copyright 2024 the JSR authors. All rights reserved. MIT license.

import { HttpError } from "fresh";
import { define } from "../../../util.ts";
import { path } from "../../../utils/api.ts";
import { Token } from "../../../utils/api_types.ts";
import { AccountLayout } from "../(_components)/AccountLayout.tsx";
import twas from "twas";
import { RevokeToken } from "./(_islands)/RevokeToken.tsx";
import TbPlus from "tb-icons/TbPlus";

export default define.page<typeof handler>(function AccountTokensPage({
  data,
}) {
  const personal = data.tokens.filter((token) => token.type === "personal");
  const sessions = data.tokens.filter((token) => token.type !== "personal");

  return (
    <AccountLayout user={data.user} active="Tokens">
      <div>
        <h2 class="text-xl mb-2 font-bold">Personal access tokens</h2>
        <p class="text-secondary max-w-2xl">
          Personal access tokens can be used to authenticate with JSR from the
          command line or from other applications.
        </p>

        {personal.length > 0
          ? (
            <ul class="max-w-2xl divide-slate-200 dark:divide-jsr-gray-700 border-t border-b border-slate-200 dark:border-jsr-gray-700 mt-4">
              {personal.map((token, idx) => (
                <PersonalTokenRow key={idx} token={token} />
              ))}
              <li class="py-2">
                <a
                  href="/account/tokens/create"
                  class="flex items-center gap-2 text-jsr-cyan-700 hover:text-jsr-cyan-600 hover:underline outline-none focus-visible:ring-2 ring-jsr-cyan-700 ring-offset-2 rounded-sm"
                >
                  <TbPlus />
                  Create new token
                </a>
              </li>
            </ul>
          )
          : (
            <div class="mt-6">
              <p class="italic text-secondary">
                You have no personal access tokens.
              </p>
              <p class="mt-2">
                <a
                  href="/account/tokens/create"
                  class="flex items-center gap-2 text-jsr-cyan-700 hover:text-jsr-cyan-600 hover:underline outline-none focus-visible:ring-2 ring-jsr-cyan-700 ring-offset-2 rounded-sm"
                >
                  <TbPlus />
                  Create new token
                </a>
              </p>
            </div>
          )}
      </div>
      <div class="mt-8">
        <h2 class="text-xl mt-4 mb-2 font-bold">Sessions</h2>
        <p class="text-secondary max-w-2xl">
          Sessions keep you logged in to JSR on the web, and are used during
          interactive authentication during publishing.
        </p>

        <ul class="max-w-2xl divide-slate-200 dark:divide-jsr-gray-700 border-t border-b border-slate-200 dark:border-jsr-gray-700 mt-4">
          {sessions.map((token, idx) => <SessionRow key={idx} token={token} />)}
        </ul>

        <p class="text-secondary text-sm mt-4">
          Only sessions that are active, or have expired within the last 24
          hours are shown here.
        </p>
      </div>
    </AccountLayout>
  );
});

function PersonalTokenRow({ token }: { token: Token }) {
  const expiresAt = token.expiresAt ? new Date(token.expiresAt) : null;
  const isActive = expiresAt === null || expiresAt.getTime() > Date.now();
  const expiresSoon = isActive &&
    (expiresAt === null ||
      expiresAt.getTime() < Date.now() + 1000 * 60 * 60 * 24 * 3);

  return (
    <li class="py-2">
      <div class="flex justify-between">
        <p class="text-secondary">
          {token.description || <i>Unnamed</i>}
        </p>
        <p class="text-sm text-right">
          <RevokeToken id={token.id} />
        </p>
      </div>
      <div class="grid sm:grid-cols-2">
        <p class="text-sm">
          {isActive
            ? (
              <span
                class={expiresSoon ? "text-orange-700" : "text-green-700"}
              >
                <b>Active</b> {expiresAt === null
                  ? "forever"
                  : `– expires ${
                    twas(new Date().getTime(), expiresAt.getTime()).replace(
                      "ago",
                      "from now",
                    )
                  }`}
              </span>
            )
            : (
              <span class="text-red-600">
                <b>Inactive</b> - expired {twas(expiresAt.getTime())}
              </span>
            )}
        </p>
        <p class="text-sm sm:text-right">
          Created {twas(new Date(token.createdAt).getTime())}
        </p>
      </div>
      <p class="text-sm text-secondary">
        {token.permissions === null
          ? "Has full access"
          : token.permissions.map((perm) => {
            if (perm.permission === "package/publish") {
              return `Can publish ${
                "package" in perm
                  ? `new versions of @${perm.scope}/${perm.package}`
                  : `new versions of any package in @${perm.scope}`
              }`;
            }
            return `has unknown permission: ${perm.permission}`;
          }).join(", ")}
      </p>
    </li>
  );
}

function SessionRow({ token }: { token: Token }) {
  const expiresAt = token.expiresAt ? new Date(token.expiresAt) : null;
  const isActive = expiresAt === null || expiresAt.getTime() > Date.now();

  return (
    <li class="py-2">
      <p class="text-secondary">
        {token.type === "web" ? "Web" : token.type === "device" ? "CLI" : ""}
        {" "}
        session
      </p>
      <div class="grid sm:grid-cols-2">
        <div>
          <p class="text-sm">
            {isActive
              ? (
                <span class="text-green-700 dark:text-green-600">
                  <b>Active</b> {expiresAt === null
                    ? "forever"
                    : `– expires ${
                      twas(new Date().getTime(), expiresAt.getTime()).replace(
                        "ago",
                        "from now",
                      )
                    }`}
                </span>
              )
              : (
                <span class="text-red-600 dark:text-red-500">
                  <b>Inactive</b> - expired {twas(expiresAt.getTime())}
                </span>
              )}

            {isActive ? "" : ""}
          </p>
        </div>
        <div>
          <p class="text-sm sm:text-right">
            Created {twas(new Date(token.createdAt).getTime())}
          </p>
        </div>
      </div>
    </li>
  );
}

export const handler = define.handlers({
  async GET(ctx) {
    const [currentUser, tokensRes] = await Promise.all([
      ctx.state.userPromise,
      ctx.state.api.get<Token[]>(path`/user/tokens`),
    ]);
    if (currentUser instanceof Response) return currentUser;
    if (!currentUser) throw new HttpError(404, "No signed in user found.");

    if (!tokensRes.ok) throw tokensRes; // gracefully handle errors

    ctx.state.meta = { title: "Your tokens - JSR" };
    return {
      data: {
        user: currentUser,
        tokens: tokensRes.data,
      },
    };
  },
});
