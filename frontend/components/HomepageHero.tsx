// Copyright 2024 the JSR authors. All rights reserved. MIT license.
import { asset } from "fresh/runtime";
import { GlobalSearch } from "../islands/GlobalSearch.tsx";
import { HomepageHeroParticles } from "../islands/HomepageHeroParticles.tsx";
import { AnimatedLogo } from "./AnimatedLogo.tsx";
import TbPlus from "@preact-icons/tb/TbPlus";

const canvasStyle = /*css*/ `.particles-js-canvas-el {
	position: absolute;
	inset: 0;
	z-index: 0;
	animation: fade-in-opacity 1s linear forwards;
	opacity: 0;
	transition: opacity 1s linear;
}
body {
	overflow-x: hidden;
}`;

export function HomepageHero(
  { apiKey, indexId }: {
    apiKey: string | undefined;
    indexId: string | undefined;
  },
) {
  return (
    <div
      class="w-screen -ml-[calc(50vw-50%)] -mt-6 bg-repeat py-32 lg:pt-48 relative before:absolute before:left-0 before:right-0 before:h-32 before:bg-gradient-to-t before:from-background-primary before:bottom-0 before:z-10 before:pointer-events-none select-none"
      id="particles-js"
    >
      <script src={asset("/scripts/particles.js")} defer></script>
      <style>{canvasStyle}</style>
      <HomepageHeroParticles />
      <div class="section-x-inset-xl flex flex-col items-center justify-center gap-12 relative pointer-events-none">
        <div class="text-center">
          <h1 class="relative z-10 flex flex-col items-center gap-6 lg:gap-8">
            <span className="sr-only">JSR</span>
            <AnimatedLogo />
            <div
              class="pointer-events-auto text-2xl text-balance leading-[1.1] sm:text-3xl md:text-3xl lg:text-4xl opsize-normal md:opsize-sm text-center -mt-5 md:-mt-6 max-w-[20em]"
              style="text-shadow: 0 0 2em hsla(var(--background-primary)), 0 0 1em hsla(var(--background-primary)), 0 0 0.5em hsla(var(--background-primary)), 0 0 0.25em hsla(var(--background-primary)), 0 0 3em hsla(var(--background-primary)), 0 0 0.5em hsla(var(--background-primary));"
            >
              The{" "}
              <b class="font-semibold">
                open-source package registry
              </b>{" "}
              for modern JavaScript and TypeScript
            </div>
          </h1>
          <div
            class="flex flex-row gap-3 items-center justify-center mt-4 pointer-events-auto"
            style="text-shadow: 0 0 2em hsla(var(--background-primary)), 0 0 1em hsla(var(--background-primary)), 0 0 0.5em hsla(var(--background-primary)), 0 0 0.25em hsla(var(--background-primary)), 0 0 3em hsla(var(--background-primary)), 0 0 0.5em hsla(var(--background-primary));"
          >
            <a class="underline text-sm relative z-10" href="/docs">
              Docs
            </a>
            <span class="w-px h-[1em] bg-jsr-cyan-200"></span>
            <a class="underline text-sm relative z-10" href="#why-jsr">
              Why JSR?
            </a>
            <span class="w-px h-[1em] bg-jsr-cyan-200"></span>
            <a
              class="underline text-sm relative z-10"
              href="https://discord.gg/hMqvhAn9xG"
            >
              Discord
            </a>
          </div>
        </div>
        <div class="w-full md:w-3/4 relative z-20">
          <GlobalSearch
            apiKey={apiKey}
            indexId={indexId}
            jumbo
          />
        </div>
        <div class="flex flex-col items-center gap-4">
          <a
            class="button-primary relative z-10 pointer-events-auto"
            href="/new"
          >
            <TbPlus class="size-5" /> Publish a package
          </a>
        </div>
      </div>
    </div>
  );
}
