// Copyright 2024 the JSR authors. All rights reserved. MIT license.
import type { DocBlockSubtitleInterfaceCtx } from "@deno/doc/html-types";

export function DocBlockSubtitleInterface(
  { value }: DocBlockSubtitleInterfaceCtx,
) {
  if (!value.extends || value.extends.length === 0) {
    return null;
  }

  return (
    <div>
      <span class="type"> extends </span>
      {value.extends.map((ext, index) => (
        <>
          <span dangerouslySetInnerHTML={{ __html: ext }} />
          {index < value.extends!.length - 1 && <span>, </span>}
        </>
      ))}
    </div>
  );
}
