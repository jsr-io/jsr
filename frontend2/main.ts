import { FreshApp, staticFiles, fsRoutes } from "@fresh/core";

export const app = new FreshApp();

app.use(staticFiles());

await fsRoutes(app, {
  dir: Deno.cwd(),
  loadIsland: (path) => import("./islands/" + path),
  loadRoute: (path) => import("./routes/" + path),
});

if (import.meta.main) {
  await app.listen();
}
