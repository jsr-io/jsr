// Copyright 2024 the JSR authors. All rights reserved. MIT license.
import type { IndexSignatureCtx } from "@deno/doc/html-types";
import { Anchor } from "./Anchor.tsx";

export function IndexSignature(
  { signature: { id, anchor, readonly, params, ts_type } }: {
    signature: IndexSignatureCtx;
  },
) {
  return (
    <div class="anchorable text-sm" id={id}>
      <Anchor anchor={anchor} />
      {readonly && <span>readonly </span>}
      [<span
      // jsdoc rendering
      // deno-lint-ignore react-no-danger
      dangerouslySetInnerHTML={{ __html: params }} />]
      <span
        // jsdoc rendering
        // deno-lint-ignore react-no-danger
        dangerouslySetInnerHTML={{ __html: ts_type }} />
    </div>
  );
}
