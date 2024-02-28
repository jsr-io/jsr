// Copyright 2024 the JSR authors. All rights reserved. MIT license.
import { defineConfig, Plugin } from "$fresh/server.ts";
import { asset } from "$fresh/runtime.ts";
import tailwind from "$fresh/plugins/tailwind.ts";
import { join } from "$std/path/mod.ts";

export default defineConfig({
  plugins: [tailwind(), fontFixer()],
});

// This plugin reads the generated style.css file from tailwind plugin and
// replaces the font paths with paths that include asset queries for cache
// busting.
function fontFixer() {
  let outDir: string;
  return {
    name: "font-fixer",
    buildStart(config) {
      outDir = config.build.outDir;
    },
    async buildEnd() {
      const stylePath = join(outDir, "static", "styles.css");
      let styleCss = await Deno.readTextFile(stylePath);
      styleCss = styleCss.replaceAll(/url\((\/.*?\.woff2)\)/g, (_, path) => {
        console.log(path);
        return `url("${asset(path)}")`;
      });
      await Deno.writeTextFile(stylePath, styleCss);
    },
  } satisfies Plugin;
}
