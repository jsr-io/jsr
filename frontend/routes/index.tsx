// Copyright 2024 the JSR authors. All rights reserved. MIT license.
import { Handlers, PageProps, RouteConfig } from "$fresh/server.ts";
import { State } from "../util.ts";
import { path } from "../utils/api.ts";
import type { Package, PackageVersion, Stats } from "../utils/api_types.ts";
import type { PanelEntry } from "../components/ListPanel.tsx";
import { ListPanel } from "../components/ListPanel.tsx";
import { Head } from "$fresh/runtime.ts";
import { ComponentChildren } from "preact";

interface Data {
  stats: Stats;
}

export default function Home({ data }: PageProps<Data>) {
  return (
    <div class="flex flex-col">
      <Head>
        <title>
          JSR: JavaScript Registry
        </title>
      </Head>

      <div class="grid grid-cols-1 gap-8 lg:grid-cols-3">
        <ListPanel title="Featured Packages">
          {data.stats.featured.map(PackageToPanelEntry)}
        </ListPanel>
        <ListPanel title="Recent updates">
          {data.stats.updated.map(PackageVersionToPanelEntry)}
        </ListPanel>
        <ListPanel title="New Packages">
          {data.stats.newest.map(PackageToPanelEntry)}
        </ListPanel>
      </div>

      <h2
        class="font-semibold text-5xl md:text-7xl lg:text-center mt-16 md:mt-24 lg:mt-48 lg:mb-16"
        id="why-jsr"
      >
        Why{" "}
        <img
          src="/logo.svg"
          class="inline w-auto h-[1em] relative -top-[0.0375em] mr-3"
          alt="JSR"
        />?
      </h2>

      <div>
        <BenefitContainer>
          <img
            src="/logos/typescript.svg"
            alt="TypeScript logo"
            class="w-full max-w-16 lg:max-w-36 lg:col-span-2 lg:mx-auto"
          />
          <div class="col-span-3 max-w-screen-sm lg:max-w-none">
            <BenefitHeading>
              Made for <b class="font-bold">TypeScript & ESM</b>
            </BenefitHeading>
            <BenefitText>
              <p>
                JSR is designed for TypeScript. You publish TypeScript source,
                and JSR handles generating API docs, <code>.d.ts</code>{" "}
                files, and transpiling your code for cross-runtime
                compatibility.
              </p>
              <p>
                JSR packages are distributed as web-standard ECMAScript modules.
              </p>
            </BenefitText>
          </div>
        </BenefitContainer>

        <BenefitContainer>
          <div className="flex gap-5 lg:gap-8 items-center lg:order-2 lg:flex-col xl:flex-row lg:col-span-2">
            <img
              src="/logos/npm.svg"
              alt="NPM logo"
              class="w-full max-w-16 lg:max-w-28"
            />
            <img
              src="/logos/yarn.svg"
              alt="Yarn logo"
              class="w-full max-w-16 lg:max-w-28"
            />
            <img
              src="/logos/pnpm.svg"
              alt="PNPM logo"
              class="w-full max-w-16 lg:max-w-28"
            />
          </div>
          <div class="col-span-3 max-w-screen-sm lg:order-1">
            <BenefitHeading>
              <b class="font-bold">Builds on</b> npm
            </BenefitHeading>
            <BenefitText>
              <p>
                JSR isn't a replacement for the npm registry; it's a superset of
                npm.
              </p>
              <p>
                JSR modules can be used with any JavaScript package manager, and
                in any project with a <code>node_modules</code>{"  "}folder.
              </p>
            </BenefitText>
          </div>
        </BenefitContainer>

        <BenefitContainer>
          <div className="flex gap-5 lg:gap-8 items-center lg:grid lg:grid-cols-4 lg:justify-items-center lg:[&>img]:h-16 lg:[&>img]:w-auto lg:max-w-max lg:mx-auto lg:col-span-2">
            <img
              src="/logos/node.svg"
              alt="Node logo"
              class="w-full max-w-9 lg:max-w-20"
            />
            <img
              src="/logos/deno.svg"
              alt="Deno logo"
              class="w-full max-w-10 lg:max-w-20"
            />
            <img
              src="/logos/bun.svg"
              alt="Bun logo"
              class="w-full max-w-11 lg:max-w-20"
            />
            <img
              src="/logos/cloudflare-workers.svg"
              alt="Cloudflare Workers logo"
              class="w-full max-w-10 lg:max-w-20"
            />
          </div>
          <div class="col-span-3 max-w-screen-sm lg:max-w-none">
            <BenefitHeading>
              Works with <b class="font-bold">any runtime</b>
            </BenefitHeading>
            <BenefitText>
              <p>
                JSR modules can be used in Node, Deno, Bun, Cloudflare Workers,
                and more.
              </p>
              <p>
                Module authors can count on great editor support from strongly
                typed modules, without the need to transpile and distribute
                typings manually.
              </p>
            </BenefitText>
          </div>
        </BenefitContainer>
      </div>

      <div class="lg:text-center my-32 lg:my-48">
        <h2 class="text-4xl md:text-5xl mb-6">Still curious about JSR?</h2>
        <a class="button-primary" href="/docs/why">
          Learn more&ensp;&rsaquo;
        </a>
      </div>
    </div>
  );
}

function BenefitContainer({ children }: { children: ComponentChildren }) {
  return (
    <div class="space-y-4 py-16 lg:py-24 border-b-1.5 border-cyan-900/10 lg:space-y-6 lg:grid lg:grid-cols-5 lg:gap-16 lg:items-center">
      {children}
    </div>
  );
}

function BenefitHeading({ children }: { children: ComponentChildren }) {
  return (
    <h3 class="text-3xl font-light md:text-4xl lg:text-5xl mb-6 text-balance">
      {children}
    </h3>
  );
}

function BenefitText({ children }: { children: ComponentChildren }) {
  return (
    <div class="text-xl space-y-4 md:text-2xl lg:text-[1.75rem] lg:leading-snug text-jsr-gray-400">
      {children}
    </div>
  );
}

function PackageToPanelEntry(
  entry: Package,
): PanelEntry {
  return {
    value: `@${entry.scope}/${entry.name}`,
    href: `/@${entry.scope}/${entry.name}`,
  };
}

function PackageVersionToPanelEntry(
  entry: PackageVersion,
): PanelEntry {
  return {
    value: `@${entry.scope}/${entry.package}`,
    href: `/@${entry.scope}/${entry.package}@${entry.version}`,
    label: entry.version,
  };
}

export const handler: Handlers<Data, State> = {
  async GET(_req, ctx) {
    const statsResp = await ctx.state.api.get<Stats>(path`/stats`);
    if (!statsResp.ok) throw statsResp; // gracefully handle this
    return ctx.render({ stats: statsResp.data });
  },
};

export const config: RouteConfig = {
  csp: true,
};
