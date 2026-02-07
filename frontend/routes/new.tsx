// Copyright 2024 the JSR authors. All rights reserved. MIT license.
import { useSignal } from "@preact/signals";
import { TbBrandGithub, TbFolder, TbPackage } from "tb-icons";
import {
  CreatePackage,
  IconCircle,
  PackageName,
  ScopeSelect,
} from "../islands/new.tsx";
import { Package, Scope } from "../utils/api_types.ts";
import { assertOk, path } from "../utils/api.ts";
import { define } from "../util.ts";

export default define.page<typeof handler>(function New(props) {
  const scope = useSignal(props.data.scope);
  const name = useSignal(props.data.newPackage ?? "");
  const pkg = useSignal<Package | null | undefined>(undefined);

  const loginUrl = props.url.pathname + props.url.search;

  return (
    <>
      <div class="flex flex-col md:grid md:grid-cols-2 gap-12">
        <div class="w-full space-y-4 flex-shrink-0">
          <h1 class="mb-8 font-bold text-3xl leading-none">
            Publish a package
          </h1>
          <p class="max-w-screen-md">
            Publish your package to the JSR to share it with the world!
          </p>
          <p>
            <b>JSR natively supports TypeScript</b>{" "}
            . You do not need to compile your package before publishing.
          </p>
          <p>
            JSR packages can depend on other JSR{" "}
            packages, as well as any package from npm.
          </p>
        </div>
        <div class="space-y-8">
          <div class="flex items-start gap-4">
            <IconCircle done={scope}>
              <TbFolder class="h-5 w-5" />
            </IconCircle>
            <div class="w-full">
              <h2 class="font-bold text-2xl leading-none">Scope</h2>
              <p class="mt-2 mb-4 text-tertiary text-base">
                Choose which scope your package will be published to. Scopes are
                namespaces for packages.
              </p>
              {props.state.user
                ? (
                  <ScopeSelect
                    scope={scope}
                    scopes={props.data.scopes}
                    initialScope={props.data.initialScope}
                    scopeUsage={props.state.user.scopeUsage}
                    scopeLimit={props.state.user.scopeLimit}
                    locked={props.data.fromCli}
                    user={props.state.user}
                  />
                )
                : (
                  <div class="space-y-4 bg-jsr-gray-50 dark:bg-jsr-gray-900 border-jsr-gray-900 dark:border-jsr-gray-50 p-4 rounded-xl">
                    <p class="text-jsr-gray-700 dark:text-white">
                      You must be logged in to publish a package.
                    </p>
                    <a
                      href={`/login?redirect=${encodeURIComponent(loginUrl)}`}
                      class="button-primary"
                    >
                      <TbBrandGithub /> Sign in with GitHub
                    </a>
                  </div>
                )}
            </div>
          </div>
          <div class="flex items-start gap-4">
            <IconCircle done={name}>
              <TbPackage class="h-5 w-5" />
            </IconCircle>
            <div class="w-full">
              <h2 class="font-bold text-2xl leading-none">Package name</h2>
              <p class="mt-1 mb-4 text-tertiary text-base">
                The name of your package must be unique within the scope you
                selected.
              </p>
              <PackageName
                scope={scope}
                name={name}
                pkg={pkg}
                locked={props.data.fromCli}
              />
              <CreatePackage
                scope={scope}
                name={name}
                pkg={pkg}
                fromCli={props.data.fromCli}
              />
            </div>
          </div>
        </div>
      </div>
    </>
  );
});

export const handler = define.handlers({
  async GET(ctx) {
    let newPackage = undefined;
    const scopesResp = await (ctx.state.api.hasToken()
      ? ctx.state.api.get<Scope[]>(path`/user/scopes`)
      : Promise.resolve(null));
    if (scopesResp) {
      assertOk(scopesResp);
    }
    const scopes = scopesResp?.data.map((scope) =>
      scope.scope
    ) ?? [];
    let scope = "";
    let initialScope;
    if (ctx.url.searchParams.has("scope")) {
      initialScope = ctx.url.searchParams.get("scope") ?? undefined;
      if (initialScope && scopes.includes(initialScope)) {
        scope = initialScope;
        initialScope = undefined;
      }
    }
    if (ctx.url.searchParams.has("package")) {
      newPackage = ctx.url.searchParams.get("package")!;
    }
    const fromCli = ctx.url.searchParams.get("from") == "cli";

    ctx.state.meta = {
      title: "Publish a package - JSR",
      description: "Create a package to publish on JSR.",
    };

    return {
      data: {
        scopes,
        scope,
        initialScope,
        newPackage,
        fromCli,
      },
    };
  },
});
