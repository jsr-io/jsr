// Copyright 2024 the JSR authors. All rights reserved. MIT license.
import { PageProps } from "$fresh";
import { asset } from "$fresh/runtime";
import { State } from "../util.ts";

export default async function App(
  { Component, state }: PageProps<undefined, State>,
) {
  const user = await state.userPromise;
  console.log("user", user);
  if (user instanceof Response) return user;
  Object.defineProperty(state, "user", { value: user });
  console.log("state", state);
  console.log("Component", Component);
  return (
    <html lang="en">
      <head>
        <meta charset="utf-8" />
        <meta name="viewport" content="width=device-width, initial-scale=1.0" />
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
