// Copyright 2024 the JSR authors. All rights reserved. MIT license.
import { defineConfig, Plugin } from "$fresh/server.ts";
import { asset } from "$fresh/runtime.ts";
import tailwind from "$fresh/plugins/tailwind.ts";
import { join } from "$std/path/mod.ts";

export default defineConfig({
  plugins: [tailwind(), assetifyCssUrl()],
});

const CSS_URL_REGEX =
  /url\((?:(?<quote>['"])(?<quoted>(?:(?!\k<quote>|\\).|\\.)*)\k<quote>|(?<unquoted>[^'")]*))\)/g;

// This plugin reads the generated style.css file from tailwind plugin and
// replaces the url() (for font paths) with paths that include asset queries for
// caching and cache busting.
function assetifyCssUrl() {
  let outDir: string;
  return {
    name: "assetify-css-url",
    buildStart(config) {
      outDir = config.build.outDir;
    },
    async buildEnd() {
      const stylePath = join(outDir, "static", "styles.css");
      let styleCss = await Deno.readTextFile(stylePath);
      styleCss = styleCss.replaceAll(CSS_URL_REGEX, (...args) => {
        const groups = args.at(-1) as Record<string, string>;
        let path: string;
        if (groups.quoted) {
          path = groups.quoted.replaceAll(/\\./g, (s) => JSON.parse(`"${s}"`));
        } else {
          path = groups.unquoted;
        }
        return `url(${JSON.stringify(asset(path))})`;
      });
      await Deno.writeTextFile(stylePath, styleCss);
    },
  } satisfies Plugin;
}
