// Copyright 2024 the JSR authors. All rights reserved. MIT license.
import type { AnchorCtx } from "@deno/doc/html-types";
import TbLink from "tb-icons/TbLink";

export function Anchor({ id }: AnchorCtx) {
  return (
    <a href={`#${id}`} class="anchor" aria-label="Anchor" tabIndex={-1}>
      <TbLink class="size-4" />
    </a>
  );
}
