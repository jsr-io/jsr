// Copyright 2024 the JSR authors. All rights reserved. MIT license.
import { App, fsRoutes, staticFiles, trailingSlashes } from "fresh";
import { State } from "./util.ts";

export const app = new App<State>()
  .use(trailingSlashes("never"))
  .use(staticFiles());

await fsRoutes(app, {
  dir: "./",
  loadIsland: (path) => import(`./islands/${path}`),
  loadRoute: (path) => import(`./routes/${path}`),
});

if (import.meta.main) {
  await app.listen({ port: Number(Deno.env.get("PORT") ?? 8080) });
}
