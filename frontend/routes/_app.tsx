// Copyright 2024 the JSR authors. All rights reserved. MIT license.
import { PageProps, RouteConfig } from "$fresh/server.ts";
import { asset, SELF, useCSP } from "$fresh/runtime.ts";
import { State } from "../util.ts";

export default async function App(
  _req: Request,
  { Component, state }: PageProps<undefined, State>,
) {
  const user = await state.userPromise;
  if (user instanceof Response) return user;
  Object.defineProperty(state, "user", { value: user });
  return (
    <html lang="en">
      <head>
        <meta charset="utf-8" />
        <meta name="viewport" content="width=device-width, initial-scale=1.0" />
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
