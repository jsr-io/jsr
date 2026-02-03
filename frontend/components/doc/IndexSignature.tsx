// Copyright 2024 the JSR authors. All rights reserved. MIT license.
import type { IndexSignatureCtx } from "@deno/doc/html-types";
import { Anchor } from "./Anchor.tsx";

export function IndexSignature(
  { id, anchor, readonly, params, ts_type }: IndexSignatureCtx,
) {
  return (
    <div class="anchorable text-sm" id={id}>
      <Anchor {...anchor} />
      {readonly && <span>readonly </span>}
      [<span dangerouslySetInnerHTML={{ __html: params }} />]
      <span dangerouslySetInnerHTML={{ __html: ts_type }} />
    </div>
  );
}
