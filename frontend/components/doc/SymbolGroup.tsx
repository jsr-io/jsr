// Copyright 2024 the JSR authors. All rights reserved. MIT license.
import type { SymbolGroupCtx } from "@deno/doc/html-types";
import { Deprecated } from "./Deprecated.tsx";
import { DocBlockSubtitleClass } from "./DocBlockSubtitleClass.tsx";
import { DocBlockSubtitleInterface } from "./DocBlockSubtitleInterface.tsx";
import { Function } from "./Function.tsx";
import { SourceButton } from "./SourceButton.tsx";
import { SymbolContent } from "./SymbolContent.tsx";
import { Tag } from "./Tag.tsx";

export function SymbolGroup(
  { content: { name, symbols } }: { content: SymbolGroupCtx },
) {
  return (
    <main class="symbolGroup" id={`symbol_${name}`}>
      {symbols.map((symbol) => (
        <article>
          <div class="symbolTitle">
            <div>
              <div class="text-2xl leading-none break-all">
                <span class={`text-${symbol.kind.kind}`}>
                  {symbol.kind.title_lowercase}
                </span>
                {" "}
                <span class="font-bold">{name}</span>
              </div>
              {symbol.subtitle && (
                <div class="symbolSubtitle">
                  {symbol.subtitle.kind === "class"
                    ? <DocBlockSubtitleClass subtitle={symbol.subtitle} />
                    : <DocBlockSubtitleInterface subtitle={symbol.subtitle} />}
                </div>
              )}
              {symbol.tags && symbol.tags.length > 0 && (
                <div class="space-x-2 !mt-2">
                  {symbol.tags.map((tag, i) => (
                    <Tag key={i} tag={tag} large />
                  ))}
                </div>
              )}
            </div>

            {symbol.source_href && (
              <SourceButton href={symbol.source_href} />
            )}
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
