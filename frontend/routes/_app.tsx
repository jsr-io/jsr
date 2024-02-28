// Copyright 2024 the JSR authors. All rights reserved. MIT license.
import { PageProps } from "$fresh/server.ts";
import { asset } from "$fresh/runtime.ts";
import { State } from "../util.ts";

export default async function App(
  _req: Request,
  { Component, state, url }: PageProps<undefined, State>,
) {
  const user = await state.userPromise;
  if (user instanceof Response) return user;
  Object.defineProperty(state, "user", { value: user });
  return (
    <html lang="en">
      <head>
        <meta charset="utf-8" />
        <meta name="viewport" content="width=device-width, initial-scale=1.0" />
        <meta
          name="description"
          content="JSR is the open-source package registry for modern JavaScript. JSR natively supports TypeScript, and works with all JS runtimes and package managers."
        />
        <meta property="og:image" content="/images/og-image.webp" />
        <link
          rel="preload"
          href="/fonts/DMSans/DMSans-Variable.woff2"
          as="font"
          type="font/woff2"
          crossOrigin="true"
        />
        <link rel="stylesheet" href={asset("/styles.css")} />
        <link rel="stylesheet" href={asset("/gfm.css")} />
        <link
          rel="icon"
          type="image/svg+xml"
          href={asset("/logo-square.svg")}
        />
      </head>
      <body>
        <Component />
      </body>
    </html>
  );
}
