// Copyright 2024 the JSR authors. All rights reserved. MIT license.
import { Marked, render } from "$gfm";
import { markedSmartypants } from "$marked-smartypants";

import "https://esm.sh/prismjs@1.29.0/components/prism-json?no-check";
import "https://esm.sh/prismjs@1.29.0/components/prism-typescript?no-check";
import "https://esm.sh/prismjs@1.29.0/components/prism-jsx?no-check";
import "https://esm.sh/prismjs@1.29.0/components/prism-tsx?no-check";
import "https://esm.sh/prismjs@1.29.0/components/prism-bash?no-check";
import "https://esm.sh/prismjs@1.29.0/components/prism-diff?no-check";

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
    <main
      data-color-mode="auto"
      data-light-theme="light"
      data-dark-theme="dark"
      class="markdown-body"
      dangerouslySetInnerHTML={{ __html: html }}
    />
  );
}
