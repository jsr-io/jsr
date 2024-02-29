// Copyright 2024 the JSR authors. All rights reserved. MIT license.
import { PageProps } from "$fresh/server.ts";
import { Header } from "../components/Header.tsx";
import { State } from "../util.ts";

export default function Layout(
  { Component, state, url }: PageProps<undefined, State>,
) {
  return (
    <>
      <div
        class="min-h-[calc(100vh-3rem)]"
        data-dark-theme="light"
      >
        <a
          href="#main-content"
          class="absolute p-4 -translate-y-full bg-cyan-100 font-medium focus:translate-y-0 transition-transform duration-100	z-20"
        >
          Skip to main content
        </a>
        <Header user={state.user} url={url} />
        <div
          class="section-x-inset-xl py-4 md:py-6 focus-visible:ring-0 focus-visible:outline-none"
          id="main-content"
          tabIndex={-1}
        >
          <Component />
        </div>
      </div>
      <footer class="text-xs text-center p-4 text-gray-500">
        JSR - It is {new Date().toLocaleString("en-ZA", {
          timeZoneName: "short",
          timeZone: "Etc/UTC",
        })} -{" "}
        <a
          href="/docs"
          class="text-cyan-700 hover:text-blue-400 underline"
        >
          Docs
        </a>
        {state?.span.isSampled ? `— x-deno-ray: ${state.span.traceId}` : null}
      </footer>
    </>
  );
}
