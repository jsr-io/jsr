// Copyright 2024 the JSR authors. All rights reserved. MIT license.
import type { ExampleCtx } from "@deno/doc/html-types";
import { Anchor } from "./Anchor.tsx";

export function Example({ anchor, markdown_title, markdown_body }: ExampleCtx) {
  return (
    <div class="anchorable">
      <Anchor {...anchor} />
      <h3
        class="example-header"
        dangerouslySetInnerHTML={{ __html: markdown_title }}
      />
      <div dangerouslySetInnerHTML={{ __html: markdown_body }} />
    </div>
  );
}
