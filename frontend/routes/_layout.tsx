// Copyright 2024 the JSR authors. All rights reserved. MIT license.
import { PageProps } from "$fresh/server.ts";
import { Header } from "../components/Header.tsx";
import { HomepageHero } from "../islands/HomepageHero.tsx";
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
        <Header user={state.user} url={url} />
        {url.pathname === "/" && (
          <HomepageHero
            apiKey={Deno.env.get("ORAMA_PUBLIC_API_KEY")}
            indexId={Deno.env.get("ORAMA_PUBLIC_INDEX_ID")}
          />
        )}
        <div class="section-x-inset-xl py-4 md:py-6">
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
