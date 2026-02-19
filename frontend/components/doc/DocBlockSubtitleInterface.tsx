// Copyright 2024 the JSR authors. All rights reserved. MIT license.
// deno-lint-ignore-file jsx-curly-braces
import type { DocBlockSubtitleInterfaceCtx } from "@deno/doc/html-types";

export function DocBlockSubtitleInterface(
  { subtitle: { value } }: { subtitle: DocBlockSubtitleInterfaceCtx },
) {
  const hasRemovals = value.extends_removed && value.extends_removed.length > 0;
  const hasExtends = value.extends && value.extends.length > 0;

  if (!hasRemovals && !hasExtends) {
    return null;
  }

  return (
    <>
      {hasRemovals && (
        <div class={`diff-removed rounded px-1 py-0.5`}>
          <span class="text-stone-400 italic dark:text-stone-500">
            {" extends "}
          </span>
          {value.extends_removed!.map((ext, index) => (
            <>
              <span
                // includes type defs which are generated with spans
                // deno-lint-ignore react-no-danger
                dangerouslySetInnerHTML={{ __html: ext }}
              />
              {index < value.extends_removed!.length - 1 && <span>{", "}</span>}
            </>
          ))}
        </div>
      )}

      {hasExtends && (
        <div>
          <span class="text-stone-400 italic dark:text-stone-500">
            {" extends "}
          </span>
          {value.extends.map((ext, index) => (
            <>
              <span
                class={value.extends_added?.includes(ext)
                  ? `diff-added rounded px-0.5`
                  : ""}
                // includes type defs which are generated with spans
                // deno-lint-ignore react-no-danger
                dangerouslySetInnerHTML={{ __html: ext }}
              />
              {index < value.extends!.length - 1 && <span>{", "}</span>}
            </>
          ))}
        </div>
      )}
    </>
  );
}
