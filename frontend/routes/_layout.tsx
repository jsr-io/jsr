// Copyright 2024 the JSR authors. All rights reserved. MIT license.
import { PageProps } from "fresh";
import { Header } from "../components/Header.tsx";
import { State } from "../util.ts";

export default function Layout(
  { Component, state, url }: PageProps<undefined, State>,
) {
  const currentDatetime = new Date();
  return (
    <>
      <div
        class="min-h-[calc(100vh-4rem)] md:min-h-[calc(100vh-4.5rem)]"
        data-dark-theme="light"
      >
        <a
          href="#main-content"
          class="absolute p-4 -translate-y-full bg-jsr-cyan-100 font-medium focus:translate-y-0 transition-transform duration-100	z-20"
        >
          Skip to main content
        </a>
        <Header
          user={state.user}
          sudo={state.sudo}
          searchKind={state.searchKind}
          url={url}
        />
        <div
          class="section-x-inset-xl pt-4 md:pt-6 focus-visible:ring-0 focus-visible:outline-none"
          id="main-content"
          tabIndex={-1}
        >
          <Component />
        </div>
      </div>
      <footer
        id="footer"
        class="text-xs text-center mt-4 md:mt-6 p-4 text-jsr-gray-500"
      >
        JSR - It is{" "}
        <time datetime={currentDatetime.toISOString()}>
          {currentDatetime.toLocaleString("en-ZA", {
            timeZoneName: "short",
            timeZone: "Etc/UTC",
          })}
        </time>{" "}
        -{" "}
        <a
          href="/docs"
          class="text-jsr-cyan-700 hover:text-blue-400 underline"
        >
          Docs
        </a>{" "}
        -{" "}
        <a
          href="https://github.com/jsr-io/jsr"
          class="text-jsr-cyan-700 hover:text-blue-400 underline"
        >
          <span>GitHub</span>
        </a>{" "}
        -{" "}
        <a
          href="https://discord.gg/hMqvhAn9xG"
          class="text-jsr-cyan-700 hover:text-blue-400 underline"
        >
          <span>Discord</span>
        </a>
        {state.span?.isSampled ? ` — x-deno-ray: ${state.span.traceId}` : null}
      </footer>
    </>
  );
}
