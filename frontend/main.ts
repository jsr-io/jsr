// Copyright 2024 the JSR authors. All rights reserved. MIT license.
import { App, fsRoutes, staticFiles, trailingSlashes } from "@fresh/core";
import { State } from "./util.ts";

export const app = new App<State>()
  .use(trailingSlashes("never"))
  .use(staticFiles());

console.time("route loading");
await fsRoutes(app, {
  dir: Deno.cwd(),
  loadIsland: (path) => import(`./islands/${path}`),
  loadRoute: (path) => import(`./routes/${path}`),
});
console.timeEnd("route loading");

if (import.meta.main) {
  await app.listen();
}
