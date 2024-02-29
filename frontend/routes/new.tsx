// Copyright 2024 the JSR authors. All rights reserved. MIT license.
import { Handlers, PageProps } from "$fresh/server.ts";
import { useSignal } from "@preact/signals";
import IconFolder from "$tabler_icons/folder.tsx";
import IconPackage from "$tabler_icons/package.tsx";
import {
  CreatePackage,
  IconCircle,
  PackageName,
  ScopeSelect,
} from "../islands/new.tsx";
import { State } from "../util.ts";
import { Package, Scope } from "../utils/api_types.ts";
import { path } from "../utils/api.ts";
import { Head } from "$fresh/runtime.ts";
import { GitHub } from "../components/icons/GitHub.tsx";

interface Data {
  scopes: string[];
  scope: string;
  initialScope?: string;
  newPackage?: string;
  fromCli: boolean;
}

export default function New(props: PageProps<Data, State>) {
  const scope = useSignal(props.data.scope);
  const name = useSignal(props.data.newPackage ?? "");
  const pkg = useSignal<Package | null | undefined>(undefined);

  const loginUrl = props.url.pathname + props.url.search;

  return (
    <>
      <Head>
        <title>
          Publish a package - JSR
        </title>
        <meta
          name="description"
          content="Create a package to publish on JSR."
        />
      </Head>
      <div class="flex flex-col md:flex-row gap-12 lg:gap-36">
        <div class="md:w-88 space-y-4 flex-shrink-0">
          <h1 class="mb-8 font-bold text-3xl leading-none">
            Publishing a package
          </h1>
          <p class="text-gray-900 max-w-screen-md">
            Publish your package to the <b>JSR</b>{" "}
            registry to share it with the world.
          </p>
          <p>
            <b>JSR</b> natively supports <b>TypeScript</b>{" "}
            packages. You do not need to compile your package before publishing.
          </p>
          <p>
            <b>JSR</b> packages can depend on other <b>JSR</b>{" "}
            packages, as well as any package from <b>npm</b>.
          </p>
        </div>
        <div class="space-y-8">
          <div class="flex items-start gap-4">
            <IconCircle done={scope}>
              <IconFolder class="h-5 w-5" />
            </IconCircle>
            <div class="w-full">
              <p class="font-bold text-xl leading-none">Scope</p>
              <p class="mt-1 mb-4 text-gray-500">
                Choose which scope you want to publish your package to. Scopes
                are namespaces for packages.
              </p>
              {props.state.user
                ? (
                  <ScopeSelect
                    scope={scope}
                    scopes={props.data.scopes}
                    initialScope={props.data.initialScope}
                    scopeUsage={props.state.user.scopeUsage}
                    scopeLimit={props.state.user.scopeLimit}
                  />
                )
                : (
                  <div class="space-y-4 bg-gray-50 border-gray-100 p-4 rounded-xl">
                    <p class="text-gray-700">
                      You must be logged in to publish a package.
                    </p>
                    <a
                      href={`/login?redirect=${encodeURIComponent(loginUrl)}`}
                      class="button-primary"
                    >
                      <GitHub /> Sign in with GitHub
                    </a>
                  </div>
                )}
            </div>
          </div>
          <div class="flex items-start gap-4">
            <IconCircle done={name}>
              <IconPackage class="h-5 w-5" />
            </IconCircle>
            <div class="w-full">
              <p class="font-bold text-xl leading-none">Package name</p>
              <p class="mt-1 mb-4 text-gray-500">
                The name of your package must be unique within the scope you
                selected.
              </p>
              <PackageName scope={scope} name={name} pkg={pkg} />

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
}

export const handler: Handlers<Data, State> = {
  async GET(req, ctx) {
    let newPackage = undefined;
    const scopesResp =
      await (ctx.state.api.hasToken()
        ? ctx.state.api.get<Scope[]>(path`/user/scopes`)
        : Promise.resolve(null));
    if (scopesResp && !scopesResp.ok) throw scopesResp; // gracefully handle this
    const scopes = scopesResp?.data.map((scope) => scope.scope) ?? [];
    const url = new URL(req.url);
    let scope = "";
    let initialScope;
    if (url.searchParams.has("scope")) {
      initialScope = url.searchParams.get("scope") ?? undefined;
      if (initialScope && scopes.includes(initialScope)) {
        scope = initialScope;
        initialScope = undefined;
      }
    }
    if (url.searchParams.has("package")) {
      newPackage = url.searchParams.get("package")!;
    }
    const fromCli = url.searchParams.get("from") == "cli";
    return ctx.render({
      scopes,
      scope,
      initialScope,
      newPackage,
      fromCli,
    });
  },
};
