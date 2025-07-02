// Copyright 2024 the JSR authors. All rights reserved. MIT license.
import { ComponentChildren } from "preact";
import twas from "twas";
import { AccountNav, AccountNavTab } from "./AccountNav.tsx";
import { FullUser, User } from "../../../utils/api_types.ts";
import { GitHubUserLink } from "../../../islands/GithubUserLink.tsx";
import { GitLabUserLink } from "../../../islands/GitLabUserLink.tsx";

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
          class="rounded-full size-16 md:size-32 lg:size-40 ring-2 ring-offset-1 ring-jsr-cyan-700"
          src={user.avatarUrl}
          alt="user icon"
        />
        <div class="max-w-60 md:max-w-32 lg:max-w-40">
          <h1 class="text-2xl leading-none font-semibold truncate">
            {user.name}
          </h1>
          <p class="text-xs text-secondary">
            Created account {twas(new Date(user.createdAt).getTime())}
          </p>
          <p class="text-base mt-3">
            <GitHubUserLink user={user} />
            {/* TODO: figure out a way to get this working, requires auth tokens to gitlab <GitLabUserLink user={user} />*/}
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
