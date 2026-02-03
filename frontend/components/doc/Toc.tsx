// Copyright 2024 the JSR authors. All rights reserved. MIT license.
import type { ToCCtx, ToCEntry } from "@deno/doc/html-types";
import TbChevronRight from "tb-icons/TbChevronRight";
import { DocNodeKindIcon } from "./DocNodeKindIcon.tsx";
import { Usages } from "./Usages.tsx";

export function Toc(
  { content: { usages, top_symbols, document_navigation } }: { content: ToCCtx },
) {
  if (!usages && !top_symbols && document_navigation.length === 0) {
    return null;
  }

  return (
    <div class="ddoc w-full lg:overflow-y-auto pb-4">
      <div class="toc">
        <div>
          {usages && (
            <Usages usages={usages.usages} composed={usages.composed} />
          )}

          {top_symbols && (
            <nav class="topSymbols">
              <h3>Symbols</h3>
              <ul>
                {top_symbols.symbols.map((symbol) => (
                  <li>
                    <a href={symbol.href} title={symbol.name}>
                      <DocNodeKindIcon kinds={symbol.kind} />
                      <span
                        class={`hover:bg-${symbol.kind[0]?.kind}/15 hover:bg-${symbol.kind[0]?.kind}Dark/15`}
                      >
                        {symbol.name}
                      </span>
                    </a>
                  </li>
                ))}
              </ul>
              {top_symbols.total_symbols > 5 && (
                <a
                  class="flex items-center gap-0.5"
                  href={top_symbols.all_symbols_href}
                >
                  <span class="leading-none">
                    view all {top_symbols.total_symbols} symbols
                  </span>
                  <TbChevronRight class="size-4" />
                </a>
              )}
            </nav>
          )}

          {document_navigation.length > 0 && (
            <nav class="documentNavigation">
              <h3>Document Navigation</h3>
              <ul>
                {renderToC(document_navigation, Math.min(...document_navigation.map((entry) => entry.level)), 0)[0]}
              </ul>
            </nav>
          )}
        </div>
      </div>
    </div>
  );
}

function renderToC(items: ToCEntry[], currentLevel: number, startIdx: number): [any[], number] {
  const result: any[] = [];
  let i = startIdx;

  while (i < items.length && items[i].level >= currentLevel) {
    const entry = items[i];

    if (entry.level === currentLevel) {
      const [children, nextIdx] = i + 1 < items.length && items[i + 1].level > currentLevel
        ? renderToC(items, items[i + 1].level, i + 1)
        : [[], i + 1];

      result.push(
        <li key={entry.anchor}>
          <a href={`#${entry.anchor}`} title={entry.content}>
            {entry.content}
          </a>
          {children.length > 0 && <ul>{children}</ul>}
        </li>
      );
      i = nextIdx;
    } else {
      break;
    }
  }

  return [result, i];
}
