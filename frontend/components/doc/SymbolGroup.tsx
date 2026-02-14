// Copyright 2024 the JSR authors. All rights reserved. MIT license.
import type { SymbolGroupCtx } from "../../../new_html_types.d.ts";
import { Deprecated } from "./Deprecated.tsx";
import { DocBlockSubtitleClass } from "./DocBlockSubtitleClass.tsx";
import { DocBlockSubtitleInterface } from "./DocBlockSubtitleInterface.tsx";
import { Function } from "./Function.tsx";
import { SourceButton } from "./SourceButton.tsx";
import { SymbolContent } from "./SymbolContent.tsx";
import { Tag } from "./Tag.tsx";

export function SymbolGroup(
  { content: { name, symbols, diff_status } }: { content: SymbolGroupCtx },
) {
  const isAdded = diff_status?.kind === "added";
  const isRemoved = diff_status?.kind === "removed";
  const isRenamed = diff_status?.kind === "renamed";

  const isModified = diff_status?.kind === "modified";

  let diffBg = "";
  if (isAdded) diffBg = ` diff-added rounded-lg p-4`;
  else if (isRemoved) {
    diffBg = ` diff-removed rounded-lg p-4`;
  }

  const renamedOldName = isRenamed
    ? (diff_status as { kind: "renamed"; old_name: string }).old_name
    : undefined;

  return (
    <main class={`space-y-12${diffBg}`} id={`symbol_${name}`}>
      {symbols.map((symbol) => (
        <article class="space-y-5">
          <div class="flex justify-between items-start group/sourceable relative">
            <div class="font-medium space-y-1">
              <div class="text-2xl leading-none break-all">
                <span class={`text-${symbol.kind.kind}`}>
                  {symbol.kind.title_lowercase}
                </span>{" "}
                {renamedOldName && (
                  <>
                    <span
                      class={`font-bold diff-removed rounded px-1`}
                    >
                      {renamedOldName}
                    </span>
                    <span class="mx-1 text-stone-400">{"\u2192"}</span>
                  </>
                )}
                <span
                  class={`font-bold${
                    renamedOldName
                      ? ` diff-added rounded px-1`
                      : ""
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
                <div class="space-x-2 !mt-2">
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
