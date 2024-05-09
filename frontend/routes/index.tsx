// Copyright 2024 the JSR authors. All rights reserved. MIT license.
import { Handlers, PageProps } from "$fresh/server.ts";
import { State } from "../util.ts";
import { path } from "../utils/api.ts";
import type { Package, PackageVersion, Stats } from "../utils/api_types.ts";
import type { PanelEntry } from "../components/ListPanel.tsx";
import { ListPanel } from "../components/ListPanel.tsx";
import { Head } from "$fresh/runtime.ts";
import { ComponentChildren } from "preact";
import { HomepageHero } from "../components/HomepageHero.tsx";
import { Logo } from "../components/Logo.tsx";
import { NewsCard } from "../components/NewsCard.tsx";

interface Post {
  title: string;
  description: string;
  image: string;
  url: string;
}

interface Data {
  stats: Stats;
  posts: Post[];
}

export default function Home({ data }: PageProps<Data>) {
  return (
    <div class="flex flex-col">
      <Head>
        <title>
          JSR: the JavaScript Registry
        </title>
        <meta
          name="description"
          content="JSR is the open-source package registry for modern JavaScript. JSR natively supports TypeScript, and works with all JS runtimes and package managers."
        />
        <meta property="og:image" content="/images/og-image.webp" />
      </Head>

      <HomepageHero
        apiKey={Deno.env.get("ORAMA_PACKAGE_PUBLIC_API_KEY")}
        indexId={Deno.env.get("ORAMA_PACKAGE_PUBLIC_INDEX_ID")}
      />
      {data.posts.length > 0 && (
        <section class="flex flex-col gap-4 mb-16 md:mb-32">
          <h2 class="text-3xl md:text-4xl mb-4 md:mb-8 font-semibold text-center">
            Latest updates
          </h2>
          <ul class="grid grid-cols-1 md:grid-cols-3 gap-4 md:gap-8">
            {data?.posts?.slice(0, 3).map((post) => (
              <NewsCard
                image={post.image}
                title={post.title}
                description={post.description}
                url={post.url}
              />
            ))}
          </ul>
          <a
            href="https://deno.com/blog?tag=jsr"
            class="underline block mt-4 w-full text-center"
          >
            More JSR updates <span aria-hidden="true">&rsaquo;</span>
          </a>
        </section>
      )}

      <section class="flex flex-col gap-4">
        <h2 class="text-3xl md:text-4xl mb-4 md:mb-8 font-semibold text-center">
          Packages
        </h2>
        <div class="grid grid-cols-1 gap-8 lg:grid-cols-3">
          <ListPanel title="Featured">
            {data.stats.featured.map(PackageToPanelEntry)}
          </ListPanel>
          <ListPanel title="Recently updated">
            {data.stats.updated.map(PackageVersionToPanelEntry)}
          </ListPanel>
          <ListPanel title="New to JSR">
            {data.stats.newest.map(PackageToPanelEntry)}
          </ListPanel>
        </div>
      </section>

      <h2
        class="font-semibold text-5xl md:text-7xl lg:text-center mt-16 md:mt-24 lg:mt-48 lg:mb-16"
        id="why-jsr"
      >
        Why{"  "}
        <Logo
          size="large"
          class="inline w-auto relative mr-3"
        />?
      </h2>

      <div>
        <BenefitContainer>
          <img
            loading="lazy"
            src="/logos/typescript.svg"
            alt="TypeScript logo"
            class="w-full max-w-16 lg:max-w-36 lg:col-span-2 lg:mx-auto select-none"
            draggable={false}
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
              loading="lazy"
              src="/logos/npm.svg"
              alt="npm logo"
              class="w-full max-w-16 lg:max-w-28 select-none"
              draggable={false}
            />
            <img
              loading="lazy"
              src="/logos/yarn.svg"
              alt="Yarn logo"
              class="w-full max-w-16 lg:max-w-28 select-none"
              draggable={false}
            />
            <img
              loading="lazy"
              src="/logos/pnpm.svg"
              alt="pnpm logo"
              class="w-full max-w-16 lg:max-w-28 select-none"
              draggable={false}
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
              loading="lazy"
              src="/logos/node.svg"
              alt="Node.js logo"
              class="w-full max-w-9 lg:max-w-20 select-none"
              draggable={false}
            />
            <img
              loading="lazy"
              src="/logos/deno.svg"
              alt="Deno logo"
              class="w-full max-w-10 lg:max-w-20 select-none"
              draggable={false}
            />
            <img
              loading="lazy"
              src="/logos/bun.svg"
              alt="Bun logo"
              class="w-full max-w-11 lg:max-w-20 select-none"
              draggable={false}
            />
            <img
              loading="lazy"
              src="/logos/cloudflare-workers.svg"
              alt="Cloudflare Workers logo"
              class="w-full max-w-10 lg:max-w-20 select-none"
              draggable={false}
            />
          </div>
          <div class="col-span-3 max-w-screen-sm lg:max-w-none">
            <BenefitHeading>
              Works with <b class="font-bold">any runtime</b>
            </BenefitHeading>
            <BenefitText>
              <p>
                JSR modules can be used in Node.js, Deno, Bun, Cloudflare
                Workers, and more.
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
    const statsResp = await ctx.state.api.get<Stats>(path`/stats`, undefined, {
      anonymous: true,
    });

    let posts: Post[] = [];
    try {
      const jsrPosts = await fetch("https://deno.com/blog/json?tag=JSR");
      if (jsrPosts.ok) {
        posts = await jsrPosts.json() as Post[];
      }
    } catch (e) {
      // ignore
    }

    if (!statsResp.ok) throw statsResp; // gracefully handle this
    return ctx.render({ stats: statsResp.data, posts: posts || [] }, {
      headers: ctx.state.api.hasToken()
        ? undefined
        : { "Cache-Control": "public, s-maxage=60" },
    });
  },
};
