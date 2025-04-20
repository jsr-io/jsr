// Copyright 2024 the JSR authors. All rights reserved. MIT license.

import { FullUser } from "../utils/api_types.ts";
import { GlobalSearch } from "../islands/GlobalSearch.tsx";
import { UserMenu } from "../islands/UserMenu.tsx";
import TbBrandGithub from "tb-icons/TbBrandGithub";
import { SearchKind } from "../util.ts";
import { HeaderLogo } from "../islands/HeaderLogo.tsx";
import DarkModeToggle from "../islands/DarkModeToggle.tsx";

export function Header({
  user,
  sudo,
  url,
  searchKind = "packages",
}: {
  user: FullUser | null;
  sudo: boolean;
  url: URL;
  searchKind?: SearchKind;
}) {
  const redirectUrl = `${url.pathname}${url.search}${url.hash}`;
  const loginUrl = `/login?redirect=${encodeURIComponent(redirectUrl)}`;
  const logoutUrl = `/logout?redirect=${encodeURIComponent(redirectUrl)}`;

  const oramaPackageApiKey = Deno.env.get("ORAMA_PACKAGE_PUBLIC_API_KEY");
  const oramaPackageIndexId = Deno.env.get("ORAMA_PACKAGE_PUBLIC_INDEX_ID");

  const oramaDocsApiKey = Deno.env.get("ORAMA_DOCS_PUBLIC_API_KEY");
  const oramaDocsIndexId = Deno.env.get("ORAMA_DOCS_PUBLIC_INDEX_ID");

  const oramaApiKey = searchKind === "packages"
    ? oramaPackageApiKey
    : oramaDocsApiKey;
  const oramaIndexId = searchKind === "packages"
    ? oramaPackageIndexId
    : oramaDocsIndexId;

  const isHomepage = url.pathname === "/";

  return (
    <>
      {user?.isStaff && sudo && (
        <div class="bg-red-600 text-white text-center py-1 px-1">
          DANGER: You have sudo mode enabled.
        </div>
      )}
      <div
        class={`section-x-inset-xl w-full py-4 sm:h-[72px] ${
          isHomepage
            ? "absolute z-50 top-0 left-0 right-0 bg-transparent pointer-events-none"
            : ""
        }`}
      >
        <div class="flex justify-between items-center text-base md:text-lg flex-wrap gap-4 lg:gap-8 h-full">
          {isHomepage ? <div></div> : (
            <a
              href="/"
              class="outline-none focus-visible:ring-2 ring-jsr-cyan-700"
            >
              <span className="sr-only">Home</span>
              <HeaderLogo class="h-8 flex-none" />
            </a>
          )}
          <div class="hidden sm:block grow-1 flex-1">
            {!isHomepage && (
              <GlobalSearch
                query={(url.pathname === "/packages"
                  ? url.searchParams.get("search")
                  : undefined) ?? undefined}
                apiKey={oramaApiKey}
                indexId={oramaIndexId}
                kind={searchKind}
              />
            )}
          </div>
          <div class="flex gap-2 sm:gap-4 items-center pointer-events-auto">
            {searchKind === "docs"
              ? (
                <a
                  href="/"
                  class="link-header"
                >
                  JSR Home
                </a>
              )
              : (
                <a
                  href="/packages"
                  className="link-header"
                >
                  Browse packages
                </a>
              )}
            {searchKind !== "docs" && (
              <>
                <Divider />
                <a
                  href="/docs"
                  class="link-header"
                >
                  Docs
                </a>
              </>
            )}
            <Divider />
            <DarkModeToggle />
            <Divider />
            {user
              ? <UserMenu user={user} sudo={sudo} logoutUrl={logoutUrl} />
              : (
                <a href={loginUrl} class="link-header flex items-center gap-2">
                  <TbBrandGithub class="size-5 flex-none" aria-hidden />
                  Sign in
                </a>
              )}
          </div>
        </div>
        <div class="mt-4 sm:hidden">
          {!isHomepage && (
            <GlobalSearch
              query={url.searchParams.get("search") ?? undefined}
              apiKey={oramaApiKey}
              indexId={oramaIndexId}
              kind={searchKind}
            />
          )}
        </div>
      </div>
    </>
  );
}

function Divider() {
  return (
    <span
      class="text-jsr-gray-200 dark:text-jsr-gray-700 select-none"
      aria-hidden="true"
    >
      |
    </span>
  );
}
