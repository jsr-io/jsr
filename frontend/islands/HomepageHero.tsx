// Copyright 2024 the JSR authors. All rights reserved. MIT license.
import { PackageSearch } from "./PackageSearch.tsx";
import { Logo } from "../components/Logo.tsx";
import { Plus } from "../components/icons/Plus.tsx";
import { useEffect } from "preact/hooks";
import { asset, Head } from "$fresh/runtime.ts";
import { AnimatedLogo } from "../components/AnimatedLogo.tsx";

declare global {
  interface Window {
    particlesJS: {
      load: (id: string, path: object, callback: () => void) => void;
    };
  }
}

const config = {
  particles: {
    number: {
      value: 56,
      density: {
        enable: true,
        value_area: 2084,
      },
    },
    color: {
      value: [
        "#22d3ee",
        "#ffd100",
        "#0e7490",
        "#a5f3fc",
        "#083344",
        "#cffafe",
        "#cbd5e1",
      ],
    },
    shape: {
      type: "polygon",
      stroke: {
        width: 0,
        color: "#22d3ee",
      },
      polygon: {
        nb_sides: 4,
      },
    },
    opacity: {
      value: 1,
      random: false,
    },
    size: {
      value: 14,
      random: true,
    },
    line_linked: {
      enable: true,
      distance: 160,
      color: "#22d3ee",
      opacity: 1,
      width: 1,
    },
    move: {
      enable: true,
      speed: 0.6,
      direction: "top",
      random: false,
      straight: false,
      out_mode: "out",
      bounce: false,
    },
  },
  interactivity: {
    detect_on: "canvas",
    events: {
      onhover: {
        enable: true,
        mode: "grab",
      },
      onclick: {
        enable: true,
        mode: "push",
      },
      resize: true,
    },
    modes: {
      grab: {
        distance: 140,
        line_linked: {
          opacity: 1,
        },
      },
      bubble: {
        distance: 400,
        size: 40,
        duration: 2,
        opacity: 8,
        speed: 3,
      },
      repulse: {
        distance: 200,
        duration: 0.4,
      },
      push: {
        particles_nb: 1,
      },
      remove: {
        particles_nb: 2,
      },
    },
  },
  retina_detect: true,
};

const canvasStyle = `.particles-js-canvas-el {
	position: absolute;
	inset: 0;
	z-index: 0;
	animation: fade-in-opacity 1s linear forwards;
	opacity: 0;
	transition: opacity 1s linear;
}`;

export function HomepageHero(
  { apiKey, indexId }: {
    apiKey: string | undefined;
    indexId: string | undefined;
  },
) {
  useEffect(() => {
    const reducedMotionQuery = globalThis.matchMedia(
      "(prefers-reduced-motion: reduce)",
    );
    const prefersReducedMotion = reducedMotionQuery.matches;
    if (prefersReducedMotion) config.particles.move.speed = 0;

    window.particlesJS.load(
      "particles-js",
      config,
      () => {
        const canvas = document.querySelector(
          ".particles-js-canvas-el",
        ) as HTMLCanvasElement;
        canvas.style.opacity = "1";
      },
    );
  }, []);
  return (
    <div
      class="bg-repeat py-32 lg:pt-48 relative before:absolute before:left-0 before:right-0 before:h-32 before:bg-gradient-to-t before:from-white before:bottom-0 before:z-10 before:pointer-events-none"
      id="particles-js"
    >
      <Head>
        <script src={asset("/scripts/particles.js")}></script>
      </Head>
      <style>{canvasStyle}</style>
      <div class="section-x-inset-xl flex flex-col items-center justify-center gap-12 relative">
        <div class="text-center">
          <h1 class="relative z-10 flex flex-col items-center gap-6 lg:gap-8">
            <span className="sr-only">JSR</span>
            <div class="h-24 md:h-36">
              <AnimatedLogo />
            </div>
            <div
              class="text-xl text-balance leading-tight sm:text-2xl md:text-3xl lg:text-4xl opsize-normal md:opsize-sm text-center"
              style="text-shadow: 0 0 2em white, 0 0 1em white, 0 0 0.5em white, 0 0 0.25em white, 0 0 3em white, 0 0 0.5em white;"
            >
              Discover, build, and share code built on web standards
            </div>
          </h1>
          <div class="flex flex-row gap-4 align-middle justify-center mt-2">
            <a class="underline text-sm relative z-10" href="/docs">
              Docs
            </a>
            <a class="underline text-sm relative z-10" href="#why-jsr">
              Why JSR?
            </a>
          </div>
        </div>
        <div class="w-full md:w-3/4 relative z-20">
          <PackageSearch
            apiKey={apiKey}
            indexId={indexId}
            jumbo={true}
          />
        </div>
        <div class="flex flex-col items-center gap-4">
          <a class="button-primary relative z-10" href="/new">
            <Plus /> Publish a package
          </a>
        </div>
      </div>
    </div>
  );
}
