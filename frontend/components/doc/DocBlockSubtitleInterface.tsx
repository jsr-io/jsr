// Copyright 2024 the JSR authors. All rights reserved. MIT license.
// deno-lint-ignore-file jsx-curly-braces
import type { DocBlockSubtitleInterfaceCtx } from "@deno/doc/html-types";

export function DocBlockSubtitleInterface(
  { subtitle: { value } }: { subtitle: DocBlockSubtitleInterfaceCtx },
) {
  if (!value.extends || value.extends.length === 0) {
    return null;
  }

  return (
    <div>
      <span class="text-stone-400 italic dark:text-stone-500">
        {" extends "}
      </span>
      {value.extends.map((ext, index) => (
        <>
          <span
            // includes type defs which are generated with spans
            // deno-lint-ignore react-no-danger
            dangerouslySetInnerHTML={{ __html: ext }}
          />
          {index < value.extends!.length - 1 && <span>{", "}</span>}
        </>
      ))}
    </div>
  );
}
