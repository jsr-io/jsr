import { FreshApp, freshStaticFiles, fsRoutes } from "@fresh/core";

export const app = new FreshApp();

app.use(freshStaticFiles());

await fsRoutes(app, {
  dir: Deno.cwd(),
  loadIsland: (path) => import("./islands/" + path),
  loadRoute: (path) => import("./routes/" + path),
});

if (import.meta.main) {
  await app.listen();
}
