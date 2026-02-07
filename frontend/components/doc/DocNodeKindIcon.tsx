// Copyright 2024 the JSR authors. All rights reserved. MIT license.
import type { DocNodeKindCtx } from "@deno/doc/html-types";

export function DocNodeKindIcon({ kinds }: { kinds: DocNodeKindCtx[] }) {
  return (
    <div class="docNodeKindIcon">
      {kinds.map((kind) => (
        <div
          class={`text-${kind.kind} bg-${kind.kind}/15 dark:text-${kind.kind}Dark dark:bg-${kind.kind}Dark/15`}
          title={kind.title}
        >
          {kind.char}
        </div>
      ))}
    </div>
  );
}
