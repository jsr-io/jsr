// Copyright 2024 the JSR authors. All rights reserved. MIT license.
import { PageProps } from "fresh";
import { asset } from "fresh/runtime";
import { State } from "../util.ts";

const FRONTEND_ROOT = Deno.env.get("FRONTEND_ROOT") ?? "http://jsr.test";

export default async function App({
  Component,
  state,
  url,
}: PageProps<undefined, State>) {
  const user = await state.userPromise;
  if (user instanceof Response) return user;
  Object.defineProperty(state, "user", { value: user });
  return (
    <html lang="en">
      <head>
        <meta charset="utf-8" />
        <meta name="viewport" content="width=device-width, initial-scale=1.0" />
        {state.meta?.title && (
          <>
            <title>{state.meta.title}</title>
            <meta property="og:title" content={state.meta.title} />
          </>
        )}
        {state.meta?.description && (
          <>
            <meta name="description" content={state.meta.description} />
            <meta property="og:description" content={state.meta.description} />
          </>
        )}

        <meta
          property="og:url"
          content={`${FRONTEND_ROOT}${url.pathname}`}
        />
        <meta property="og:type" content="article" />
        <meta property="og:site_name" content="JSR" />
        <meta
          property="og:image"
          content={state.meta?.ogImage ??
            `${FRONTEND_ROOT}/images/og-image.webp`}
        />
        <meta name="twitter:card" content="summary_large_image" />

        <link
          rel="preload"
          href={asset("/fonts/DMSans/DMSans-Variable.woff2")}
          as="font"
          type="font/woff2"
          crossOrigin="anonymous"
        />
        <link rel="stylesheet" href={asset("/styles.css")} />
        <link rel="stylesheet" href={asset("/gfm.css")} />
        <link
          rel="icon"
          type="image/svg+xml"
          href={asset("/logo-square.svg")}
        />
        <link
          rel="search"
          type="application/opensearchdescription+xml"
          href="/opensearch.xml"
          title="JSR"
        />
        
        {/* Initial dark mode script to prevent flash */}
        <script dangerouslySetInnerHTML={{
          __html: `
            (function() {
              const isDarkStored = localStorage.getItem('darkMode');
              const isDarkPreference = window.matchMedia('(prefers-color-scheme: dark)').matches;
              
              if (isDarkStored === 'true' || (isDarkStored === null && isDarkPreference)) {
                document.documentElement.classList.add('dark');
              } else {
                document.documentElement.classList.remove('dark');
              }
            })();
          `
        }} />
      </head>
      <body>
        <Component />
      </body>
    </html>
  );
}
