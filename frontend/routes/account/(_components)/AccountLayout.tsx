// Copyright 2024 the JSR authors. All rights reserved. MIT license.
import { ComponentChildren } from "preact";
import twas from "twas";
import { AccountNav, AccountNavTab } from "./AccountNav.tsx";
import { FullUser, User } from "../../../utils/api_types.ts";
import { GitHubUserLink } from "../../../islands/GithubUserLink.tsx";

interface AccountLayoutProps {
  user: User | FullUser;
  active: AccountNavTab;
  children: ComponentChildren;
}

export function AccountLayout({ user, active, children }: AccountLayoutProps) {
  return (
    <div class="grid grid-cols-1 md:grid-cols-4 gap-6">
      <div class="gap-4 flex flex-row md:flex-col items-center md:items-start md:pb-8 md:pt-4 min-w-0">
        <img
          class="rounded-full size-16 md:size-60 ring-2 ring-offset-1 ring-jsr-cyan-700 flex-none"
          src={user.avatarUrl}
          alt="user icon"
        />
        <div class="ml-2 flex-1 min-w-0 max-w-full">
          <div class="flex items-center gap-2 max-w-full">
            <h1 class="text-xl md:text-2xl leading-tight font-semibold truncate">
              {user.name}
            </h1>
            {user.id === "00000000-0000-0000-0000-000000000000" &&
              (
                <div class="flex-none chip bg-jsr-yellow-400 dark:text-jsr-gray-800 select-none mr-2">
                  bot
                </div>
              )}
          </div>
          <p class="text-sm text-secondary">
            Created account {twas(new Date(user.createdAt).getTime())}
          </p>
          <p class="text-sm mt-2">
            <GitHubUserLink user={user} />
          </p>
        </div>
      </div>
      <div class="md:col-span-3">
        {"inviteCount" in user &&
          <AccountNav user={user} active={active} />}
        <div class="mt-8">
          {children}
        </div>
      </div>
    </div>
  );
}
