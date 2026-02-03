// Copyright 2024 the JSR authors. All rights reserved. MIT license.
import type { ExampleCtx } from "@deno/doc/html-types";
import { Anchor } from "./Anchor.tsx";

export function Example(
  { example: { anchor, markdown_title, markdown_body } }: {
    example: ExampleCtx;
  },
) {
  return (
    <div class="anchorable">
      <Anchor anchor={anchor} />
      <h3
        class="example-header"
        // jsdoc rendering
        // deno-lint-ignore react-no-danger
        dangerouslySetInnerHTML={{ __html: markdown_title }}
      />
      <div
        // jsdoc rendering
        // deno-lint-ignore react-no-danger
        dangerouslySetInnerHTML={{ __html: markdown_body }}
      />
    </div>
  );
}
