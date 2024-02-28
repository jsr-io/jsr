// Copyright 2024 the JSR authors. All rights reserved. MIT license.
import { useEffect, useRef } from "preact/hooks";
import { Logo } from "../components/Logo.tsx";
import { FullUser } from "../utils/api_types.ts";

export function WorksWith(props: { user: FullUser | null }) {
  const RUNTIMES = [
    "Cloudflare Workers",
    "Next.js",
    "Node",
    "esbuild",
    "Vite",
    "Bun",
    "Deno",
    "Rollup",
  ];

  const runtimeEl = useRef(null);
  useEffect(() => {
    let currentRuntime = 0;
    const container = runtimeEl.current! as HTMLElement;
    const allRuntimeEls = container.querySelectorAll("[data-runtime]");

    const swapRuntime = () => {
      const thisRuntimeEl = allRuntimeEls[currentRuntime];
      const lastRuntime = currentRuntime
        ? currentRuntime - 1
        : RUNTIMES.length - 1;
      const lastRuntimeEl = allRuntimeEls[lastRuntime];

      const nextRuntimeEl =
        allRuntimeEls[(currentRuntime + 1) % RUNTIMES.length];

      thisRuntimeEl.querySelectorAll("span").forEach((span) =>
        span.classList.add("opacity-1", "translate-y-0")
      );
      thisRuntimeEl.querySelectorAll("span").forEach((span) =>
        span.classList.remove("opacity-0", "translate-y-6")
      );

      lastRuntimeEl.querySelectorAll("span").forEach((span) =>
        span.classList.remove("opacity-1", "translate-y-0")
      );
      lastRuntimeEl.querySelectorAll("span").forEach((span) =>
        span.classList.add("opacity-0", "-translate-y-6")
      );

      nextRuntimeEl.querySelectorAll("span").forEach((span) =>
        span.classList.remove("-translate-y-0")
      );
      nextRuntimeEl.querySelectorAll("span").forEach((span) =>
        span.classList.add("translate-y-6")
      );

      currentRuntime = (currentRuntime + 1) % RUNTIMES.length;
    };
    const interval = setInterval(swapRuntime, 1600);
    swapRuntime();
    return () => clearInterval(interval);
  }, []);

  return (
    <div
      class="flex flex-col sm:border-l sm:border-slate-700 pl-8 py-8 sm:py-16 gap-4"
      ref={runtimeEl}
    >
      <div>
        <h1
          class="text-2xl sm:text-4xl text-balance font-light text-gray-50 opacity-0 animate-fade-in"
          style="animation-delay: 200ms"
        >
          JavaScript Registry
        </h1>
      </div>

      <div
        class="opacity-0 animate-fade-in"
        style="animation-delay: 300ms"
      >
        <h3 class="font-mono text-sm italic text-cyan-400">
          Works with
        </h3>
        <ul class="items-center text-lg md:text-xl text-gray-100 font-mono whitespace-pre grid grid-cols-1 grid-rows-1">
          {RUNTIMES.map((runtime, runtimeIdx) => (
            <li class="col-start-1 row-start-1" data-runtime>
              {[...runtime].map((letter, letterIdx) => (
                <span
                  class={`inline-block transition-all duration-200 transform ${
                    runtimeIdx
                      ? "opacity-0 -translate-y-6"
                      : "opacity-1 translate-y-0"
                  }`}
                  style={{ transitionDelay: `${(letterIdx + 1) * 60}ms` }}
                >
                  {letter}
                </span>
              ))}
            </li>
          ))}
        </ul>
      </div>
      <div
        class="mt-6 opacity-0 animate-fade-in"
        style="animation-delay: 400ms"
      >
        {props.user
          ? (
            <div>
              <p class="text-gray-50 text-lg text-balance">
                You're on the waitlist! We'll email you soon…
              </p>
            </div>
          )
          : (
            <>
              <a
                href="/login?redirect=/waitlist"
                class="button-primary"
              >
                Join the waitlist
              </a>
              <a
                href="/login?redirect=/"
                class="block text-gray-400 text-xs hover:text-gray-100 hover:underline font-mono mt-6"
              >
                Got your invite already? <span class="underline">Sign in</span>
              </a>
            </>
          )}
        {props.user && (
          <p
            class="mt-4 text-gray-300 font-mono text-sm opacity-0 animate-fade-in"
            style="animation-delay: 500ms"
          >
            Signed in as: {props.user.email}

            <a
              href="/logout"
              class="hover:text-gray-200 mt-2 block underline"
            >
              Sign out
            </a>
          </p>
        )}
      </div>
    </div>
  );
}
