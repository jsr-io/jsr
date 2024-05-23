#!/usr/bin/env -S deno run -A --watch=static/,routes/
// Copyright 2024 the JSR authors. All rights reserved. MIT license.

import { Builder } from "@fresh/core/dev";
import { asset } from "@fresh/core/runtime";
import { tailwind } from "@fresh/plugin-tailwind";
import { app } from "./main.ts";
import { CSS } from "@deno/gfm";

const builder = new Builder();
tailwind(builder, app, {});
builder.onTransformStaticFile(
  { pluginName: "gfm-css", filter: /gfm\.css/ },
  (args) => {
    const css = CSS.replaceAll("font-size:16px;", "");
    return {
      content: args.text.replace(
        "/* During the build process, the @deno/gfm CSS file is injected here. */",
        css,
      ),
    };
  },
);

const CSS_URL_REGEX =
  /url\((?:(?<quote>['"])(?<quoted>(?:(?!\k<quote>|\\).|\\.)*)\k<quote>|(?<unquoted>[^'")]*))\)/g;

// This plugin reads the generated style.css file from tailwind plugin and
// replaces the url() (for font paths) with paths that include asset queries for
// caching and cache busting.
builder.onTransformStaticFile(
  { pluginName: "assetify-css-url", filter: /\.css/ },
  (args) => (
    {
      content: args.text.replaceAll(CSS_URL_REGEX, (...args) => {
        const groups = args.at(-1) as Record<string, string>;
        let path: string;
        if (groups.quoted) {
          path = groups.quoted.replaceAll(/\\./g, (s) => JSON.parse(`"${s}"`));
        } else {
          path = groups.unquoted;
        }
        return `url(${JSON.stringify(asset(path))})`;
      }),
    }
  ),
);

if (Deno.args.includes("build")) {
  await builder.build(app);
} else {
  await builder.listen(app);
}
