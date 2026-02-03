// Copyright 2024 the JSR authors. All rights reserved. MIT license.
import type { SymbolGroupCtx, UsagesCtx } from "@deno/doc/html-types";
import { Deprecated } from "./Deprecated.tsx";
import { DocBlockSubtitleClass } from "./DocBlockSubtitleClass.tsx";
import { DocBlockSubtitleInterface } from "./DocBlockSubtitleInterface.tsx";
import { Function } from "./Function.tsx";
import { SourceButton } from "./SourceButton.tsx";
import { SymbolContent } from "./SymbolContent.tsx";
import { Tag } from "./Tag.tsx";
import { UsagesLarge } from "./UsagesLarge.tsx";

export interface SymbolGroupProps extends SymbolGroupCtx {
  usage?: UsagesCtx;
}

export function SymbolGroup({ content: { name, symbols, usage } }: { content: SymbolGroupProps }) {
  return (
    <main class="symbolGroup" id={`symbol_${name}`}>
      {symbols.map((symbol, index) => (
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
                    ? <DocBlockSubtitleClass {...symbol.subtitle} />
                    : <DocBlockSubtitleInterface {...symbol.subtitle} />}
                </div>
              )}
              {symbol.tags && symbol.tags.length > 0 && (
                <div class="space-x-2 !mt-2">
                  {symbol.tags.map((tag, i) => <Tag key={i} value={tag} large />)}
                </div>
              )}
            </div>

            {symbol.source_href && <SourceButton href={symbol.source_href} />}
          </div>

          {usage && index === 0 && (
            <UsagesLarge usages={usage.usages} composed={usage.composed} />
          )}

          <Deprecated message={symbol.deprecated} />

          <div>
            {symbol.content.map((item, i) =>
              item.kind === "function"
                ? <Function key={i} {...item.value} />
                : <SymbolContent key={i} content={item.value} />
            )}
          </div>
        </article>
      ))}
    </main>
  );
}
