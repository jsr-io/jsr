// Copyright 2024 the JSR authors. All rights reserved. MIT license.
import type { SymbolGroupCtx } from "@deno/doc/html-types";
import { Deprecated } from "./Deprecated.tsx";
import { DocBlockSubtitleClass } from "./DocBlockSubtitleClass.tsx";
import { DocBlockSubtitleInterface } from "./DocBlockSubtitleInterface.tsx";
import { Function } from "./Function.tsx";
import { SourceButton } from "./SourceButton.tsx";
import { SymbolContent } from "./SymbolContent.tsx";
import { Tag } from "./Tag.tsx";
import { getDiffColor } from "./mod.ts";

export function SymbolGroup(
  { content: { name, symbols, diff_status } }: { content: SymbolGroupCtx },
) {
  const renamedOldName = diff_status?.kind === "renamed"
    ? diff_status.old_name
    : undefined;

  return (
    <main
      class={`space-y-12 px-3 -mx-3 py-2 -my-2 ${
        getDiffColor(diff_status, false)
      }`}
      id={`symbol_${name}`}
    >
      {symbols.map((symbol, i) => (
        <article
          key={i}
          class={`space-y-5 px-2 -mx-2 py-1 -my-1 ${
            getDiffColor(symbol.diff_status, false)
          }`}
        >
          <div class="flex justify-between items-start group/sourceable relative">
            <div class="font-medium space-y-1">
              <div class="text-2xl leading-none break-all">
                <span class={`text-${symbol.kind.kind}`}>
                  {symbol.kind.title_lowercase}
                </span>{" "}
                {renamedOldName && (
                  <span class="font-bold diff-removed diff-inline">
                    {renamedOldName}
                  </span>
                )}
                <span
                  class={`font-bold ${
                    renamedOldName ? "diff-added diff-inline" : ""
                  }`}
                >
                  {name}
                </span>
              </div>
              {symbol.subtitle && (
                <div class="space-y-0.5 text-sm leading-4">
                  {symbol.subtitle.kind === "class"
                    ? <DocBlockSubtitleClass subtitle={symbol.subtitle} />
                    : <DocBlockSubtitleInterface subtitle={symbol.subtitle} />}
                </div>
              )}
              {symbol.tags && symbol.tags.length > 0 && (
                <div class="space-x-2 mt-2!">
                  {symbol.tags.map((tag, i) => <Tag key={i} tag={tag} large />)}
                </div>
              )}
            </div>

            {symbol.source_href && <SourceButton href={symbol.source_href} />}
          </div>

          <Deprecated message={symbol.deprecated} />

          <div>
            {symbol.content.map((item, i) =>
              item.kind === "function"
                ? <Function key={i} func={item.value} />
                : <SymbolContent key={i} content={item.value} />
            )}
          </div>
        </article>
      ))}
    </main>
  );
}
