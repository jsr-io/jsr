// Copyright 2024 the JSR authors. All rights reserved. MIT license.
import type { AnchorCtx } from "@deno/doc/html-types";
import TbLink from "tb-icons/TbLink";

export function Anchor({ anchor: { id } }: { anchor: AnchorCtx }) {
  return (
    <a
      href={`#${id}`}
      class="anchor hidden float-left leading-none text-stone-600 ml-[-24px] p-1 pr-1 pt-1 top-0 bottom-0 dark:text-stone-400"
      aria-label="Anchor"
      tabIndex={-1}
    >
      <TbLink class="size-4" />
    </a>
  );
}
