// Copyright 2024 the JSR authors. All rights reserved. MIT license.
import type { DocNodeKindCtx } from "@deno/doc/html-types";

export function DocNodeKindIcon(
  { kinds, class: classes }: { kinds: DocNodeKindCtx[]; class?: string },
) {
  return (
    <div class={`inline-flex justify-end shrink-0 ${classes ?? ""}`}>
      {kinds.map((kind, i) => (
        <div
          class={`rounded-full size-4 font-medium text-xs leading-4 text-center align-middle shrink-0 select-none font-mono text-${kind.kind} bg-${kind.kind}/15 dark:text-${kind.kind}Dark dark:bg-${kind.kind}Dark/15 ${
            i !== 0 ? "-ml-1.5" : ""
          }`}
          title={kind.title}
        >
          {kind.char}
        </div>
      ))}
    </div>
  );
}
