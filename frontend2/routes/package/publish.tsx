// Copyright 2024 the JSR authors. All rights reserved. MIT license.
import { Handlers, PageProps, RouteConfig } from "$fresh";
import type { FullUser, Package, ScopeMember } from "../../utils/api_types.ts";
import { State } from "../../util.ts";
import { packageData } from "../../utils/data.ts";
import { GitHubActionsLink } from "../../islands/GitHubActionsLink.tsx";
import { PackageNav, Params } from "./(_components)/PackageNav.tsx";
import { PackageHeader } from "./(_components)/PackageHeader.tsx";
import { GitHub } from "../../components/icons/GitHub.tsx";
import { scopeIAM } from "../../utils/iam.ts";
import { ScopeIAM } from "../../utils/iam.ts";

interface Data {
  package: Package;
  iam: ScopeIAM;
}

export default function PackagePage({
  data,
  params,
  state,
}: PageProps<Data, State>) {
  return (
    <div class="mb-20">
      <PackageHeader package={data.package} />

      <PackageNav
        currentTab="Publish"
        versionCount={data.package.versionCount}
        iam={data.iam}
        params={params as unknown as Params}
        latestVersion={data.package.latestVersion}
      />
      <div class="mt-12 grid grid-cols-1 gap-4 lg:grid-cols-[1fr_auto_1fr] lg:gap-y-8 lg:gap-x-16">
        <div>
          <h2 class="font-bold text-2xl lg:text-3xl mb-8 text-balance">
            How to publish:
          </h2>
          <div class="space-y-16">
            <div
              class="w-full"
              id="config"
            >
              <h3 class="font-bold text-xl lg:text-2xl">1. Configure</h3>
              <div class="flex flex-col mt-4 gap-2">
                <p>
                  Add <code class="text-slate-500">"name"</code>,
                  <code class="text-slate-500">"version"</code>, and{" "}
                  <code class="text-slate-500">"exports"</code> fields to your
                  {" "}
                  config file:
                </p>
                <div class="mt-2 -mb-2">
                  <div class="bg-gray-700 text-white rounded-t font-mono text-sm px-2 py-0.5 inline-block select-none">
                    jsr.json / deno.json
                  </div>
                </div>
                <pre class="bg-slate-900 text-white rounded-lg rounded-tl-none p-4 mb-2 w-full max-w-full overflow-auto">
                <code>
                  {`{\n`}
                  {"  "}
                  <span class="bg-[rgba(134,239,172,.25)] text-[rgba(190,242,100)]">{`"name": "@${data.package.scope}/${data.package.name}",\n`}</span>
                  {"  "}
                  <span class="bg-[rgba(134,239,172,.25)] text-[rgba(190,242,100)]">{`"version": "0.1.0",\n`}</span>
                  {"  "}
                  <span class="bg-[rgba(134,239,172,.25)] text-[rgba(190,242,100)]">{`"exports": "./mod.ts"\n`}</span>
                  {`}`}
                </code>
                </pre>
                <p>
                  The version must be in{" "}
                  <a href="https://semver.org" class="link">
                    SemVer
                  </a>{" "}
                  format.
                </p>
                <p>
                  The exports field specifies the entry point of your package.
                  You can specify multiple entry points by using an object
                  instead of a string.{" "}
                  <a
                    href="/docs/publishing-packages#package-metadata"
                    class="link"
                  >
                    Learn more about exports.
                  </a>
                </p>
              </div>
            </div>
            <div
              class="w-full"
              id="manually"
            >
              <div>
                <h3 class="font-bold  text-xl lg:text-2xl">
                  2. Pick a publishing method
                </h3>
              </div>
            </div>
          </div>
        </div>
        <div class="lg:col-span-2"></div>
        <div>
          <h4 class="font-bold text-lg lg:text-xl">Publish via CLI</h4>
          <div class="flex flex-col mt-4 gap-2">
            <p>To publish your package from your terminal, run:</p>
            <pre class="bg-slate-900 text-white rounded-lg p-4 my-2 w-full max-w-full overflow-auto">
                  <code>
                    <span class="select-none sr-none text-gray-500">$ </span>
                    {`npx jsr publish`}
                    <br />
                    <span class="select-none sr-none text-gray-500 italic">or</span>
                    <br />
                    <span class="select-none sr-none text-gray-500">$ </span>
                    {`deno publish`}
                  </code>
            </pre>
            <p>
              You will be prompted to interactively authenticate in your
              browser.
            </p>
          </div>
        </div>
        <div class="h-full w-full grid grid-cols-1 grid-rows-1 [&>*]:col-start-1 [&>*]:row-start-1 items-center justify-center">
          <hr class="border-t-1.5 border-cyan-900 lg:border-l-1.5 lg:border-t-0 lg:h-full lg:mx-auto" />
          <div class="p-2 bg-white text-center w-max mx-auto font-bold">OR</div>
        </div>
        <div>
          <h4 class="font-bold text-lg lg:text-xl">
            Publish from CI
          </h4>
          <div class="flex flex-col mt-4 gap-2">
            <p>
              You can automatically publish your package from GitHub Actions.
            </p>
            <GitHubActions
              pkg={data.package}
              canEdit={data.iam.canWrite}
              user={state.user ?? undefined}
            />
          </div>
        </div>
      </div>
    </div>
  );
}

function GitHubActions({ pkg, canEdit, user }: {
  pkg: Package;
  canEdit: boolean;
  user?: FullUser;
}) {
  if (!pkg.githubRepository) {
    return (
      <>
        <div class="flex flex-col gap-4">
          <p>
            Link your GitHub repository to your package to enable publishing
            from GitHub Actions via an OIDC flow.{" "}
            <a
              href="/docs/publishing-packages#publishing-from-github-actions"
              class="underline"
            >
              Learn more.
            </a>
          </p>
          <p>
            You will need to run{" "}
            <code class="bg-gray-200 px-1.5 py-0.5 rounded-sm">
              deno publish
            </code>{" "}
            in your action.
          </p>

          {canEdit ? <GitHubActionsLink pkg={pkg} user={user} /> : (
            <p>
              Ask an admin of this scope to link the repository in the package
              settings.
            </p>
          )}
        </div>
      </>
    );
  }

  return (
    <>
      <p class="mt-4">
        This package is linked to <GitHub class="inline w-5 h-5 -mt-[2px]" />
        {" "}
        <a
          href={`https://github.com/${pkg.githubRepository.owner}/${pkg.githubRepository.name}`}
          class="link"
        >
          {pkg.githubRepository.owner}/{pkg.githubRepository.name}
        </a>
        . No secrets are required when publishing from GitHub Actions.
        Authentication happens automatically using{"  "}
        <a
          href="https://docs.github.com/en/actions/deployment/security-hardening-your-deployments/about-security-hardening-with-openid-connect"
          class="hover:underline"
        >
          OIDC
        </a>
        .{" "}
      </p>
      <p class="mt-4">
        Set up your workflow with OIDC permissions and a step to run{" "}
        <code class="bg-slate-900 text-white rounded py-[1px] px-2 text-sm">
          npx jsr publish
        </code>:
      </p>

      <div class="mt-2 -mb-2">
        <div class="text-white rounded-t font-mono text-sm px-2 py-0.5 inline-block">
          .github/workflows/publish.yml
        </div>
      </div>
      <pre class="bg-slate-900 text-white rounded-lg rounded-tl-none p-4 mb-2 w-full max-w-full overflow-auto">
        <code>
          {`\
name: Publish
on:
  push:
    branches:
      - main

jobs:
  publish:
    runs-on: ubuntu-latest
\n`}
          <span class="bg-[rgba(134,239,172,.25)] text-[rgba(190,242,100)]">{`    permissions:\n`}</span>
          <span class="bg-[rgba(134,239,172,.25)] text-[rgba(190,242,100)]">{`      contents: read\n`}</span>
          <span class="bg-[rgba(134,239,172,.25)] text-[rgba(190,242,100)]">{`      id-token: write\n`}</span>
          {`
    steps:
      - uses: actions/checkout@v4\n\n`}
          <span class="bg-[rgba(134,239,172,.25)] text-[rgba(190,242,100)]">{`      - name: Publish package\n`}</span>
          <span class="bg-[rgba(134,239,172,.25)] text-[rgba(190,242,100)]">{`        run: npx jsr publish\n`}</span>
        </code>
      </pre>
    </>
  );
}

export const handler: Handlers<Data, State> = {
  async GET(_, ctx) {
    const [user, data] = await Promise.all([
      ctx.state.userPromise,
      packageData(ctx.state, ctx.params.scope, ctx.params.package),
    ]);
    if (user instanceof Response) return user;
    if (!data) return ctx.renderNotFound();

    const { pkg, scopeMember } = data;

    const iam = scopeIAM(ctx.state, scopeMember, user);
    if (!iam.canWrite) return ctx.renderNotFound();

    return ctx.render({
      package: pkg,
      iam,
    });
  },
};

export const config: RouteConfig = {
  routeOverride: "/@:scope/:package/publish",
};
