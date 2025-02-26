// Copyright 2024 the JSR authors. All rights reserved. MIT license.
import { Marked, render } from "@deno/gfm";
import { markedSmartypants } from "marked-smartypants";

import "prismjs/components/prism-json.js";
import "prismjs/components/prism-typescript.js";
import "prismjs/components/prism-jsx.js";
import "prismjs/components/prism-tsx.js";
import "prismjs/components/prism-bash.js";
import "prismjs/components/prism-diff.js";

Marked.marked.use(markedSmartypants());

export function Markdown(
  { source, baseURL, mediaBaseURL }: {
    source: string;
    baseURL?: string;
    mediaBaseURL?: string;
  },
) {
  const html = render(source, {
    allowIframes: false,
    baseUrl: baseURL,
    mediaBaseUrl: mediaBaseURL,
  });
  return (
    <div
      class="markdown-body"
      // deno-lint-ignore react-no-danger
      dangerouslySetInnerHTML={{ __html: html }}
    />
  );
}
