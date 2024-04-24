#!/usr/bin/env -S deno run -A --watch=static/,routes/
import { tailwind } from "@fresh/plugin-tailwind";

import { Builder } from "@fresh/core/dev";
import { app } from "./main.ts";
import { CSS } from "$gfm";

const builder = new Builder();

builder.onTransformStaticFile(
  { filter: /^gfm\.css$/ },
  function transform(file) {
    const patchedCSS = CSS.replaceAll("font-size:16px;", "");
    const css = /*css*/ `${patchedCSS}
.markdown-body {
line-height: 1.6;
overflow: visible;
}

.markdown-body a {
text-decoration: underline;
}

.markdown-body :where(b, strong) {
font-weight: 650;
}

.markdown-body ul {
list-style: disc;
}
.markdown-body ol {
list-style: numeric;
}

@media screen and (max-width: 768px) {
.markdown-body pre,
.markdown-body .highlight pre {
  border-left: 0;
  border-right: 0;
  border-radius: 0;
}

.markdown-body pre {
  margin-inline: -1rem;
}
}

@media screen and (max-width: 1024px) {
.markdown-body.break pre > code {
  white-space: break-spaces;
  word-break: break-word;
}
}

.markdown-body table {
width: fit-content;
}

.markdown-body h2 {
padding-bottom: 0.375em;
}

.markdown-body h2,
.markdown-body h3 {
margin-top: 2em;
}

.markdown-body pre {
border: 1.5px solid #cbd5e1;
}

@media screen and (min-width: 1024px) {
.markdown-body .highlight pre,
.markdown-body pre {
  padding: 1.5rem;
}
}

.markdown-body blockquote {
padding: 1.5rem;
background: #f1f5f9; /* cyan-200 */
}

.markdown-body p,
.markdown-body blockquote,
.markdown-body ul,
.markdown-body ol,
.markdown-body dl,
.markdown-body table,
.markdown-body pre,
.markdown-body details,
.markdown-body .highlight {
margin-bottom: 1.25rem;
}
`;

    return {
      content: css,
    };
  },
);

tailwind(builder, app, {});

if (Deno.args.includes("build")) {
  await builder.build();
} else {
  await builder.listen(app);
}