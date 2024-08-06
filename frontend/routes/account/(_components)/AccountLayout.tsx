// Copyright 2024 the JSR authors. All rights reserved. MIT license.
import { FullUser, User } from "../../../utils/api_types.ts";
import { AccountNav, AccountNavTab } from "./AccountNav.tsx";
import twas from "$twas";
import { ComponentChildren } from "preact";
import { GitHubUserLink } from "../../../islands/GithubUserLink.tsx";

interface AccountLayoutProps {
  user: User | FullUser;
  active: AccountNavTab;
  children: ComponentChildren;
}

export function AccountLayout({ user, active, children }: AccountLayoutProps) {
  return (
    <div class="grid grid-cols-1 md:grid-cols-5 gap-4">
      <div class="gap-4 flex flex-row md:flex-col items-center pr-4 md:pb-8 md:pt-4">
        <img
          class="rounded-full w-16 h-16 md:h-32 md:w-32 lg:h-40 lg:w-40 ring-2 ring-offset-1 ring-jsr-gray-300"
          src={user.avatarUrl}
          alt="user icon"
        />
        <div>
          <h1 class="text-2xl leading-none font-semibold">
            {user.name}
          </h1>
          <p class="text-xs text-jsr-gray-600">
            Created account {twas(new Date(user.createdAt))}
          </p>
          <p class="text-xs text-jsr-gray-600">
            <GitHubUserLink user={user} />
          </p>
        </div>
      </div>
      <div class="md:col-span-4">
        {"inviteCount" in user &&
          <AccountNav user={user} active={active} />}
        <div class="mt-8">
          {children}
        </div>
      </div>
    </div>
  );
}
