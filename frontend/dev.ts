#!/usr/bin/env -S deno run -A --watch=static/,routes/
// Copyright 2024 the JSR authors. All rights reserved. MIT license.

import { Builder } from "fresh/dev";
import { tailwind } from "@fresh/plugin-tailwind";
import { CSS } from "@deno/gfm";

const builder = new Builder();
tailwind(builder, {});
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

if (Deno.args.includes("build")) {
  await builder.build();
} else {
  await builder.listen(() => import("./main.ts"));
}
