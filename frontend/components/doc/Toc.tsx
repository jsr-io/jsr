// Copyright 2024 the JSR authors. All rights reserved. MIT license.
import type { VNode } from "preact";
import type { ToCCtx, ToCEntry } from "@deno/doc/html-types";
import TbChevronRight from "tb-icons/TbChevronRight";
import { DocNodeKindIcon } from "./DocNodeKindIcon.tsx";
import Usages from "../../islands/DocUsages.tsx";

export function Toc(
  { content: { usages, top_symbols, document_navigation } }: {
    content: ToCCtx;
  },
) {
  if (!usages && !top_symbols && document_navigation.length === 0) {
    return null;
  }

  return (
    <div class="ddoc w-full lg:overflow-y-auto pb-4">
      <div class="toc">
        <div class="space-y-5">
          {usages && <Usages usages={usages} />}

          {top_symbols && (
            <nav class="max-lg:hidden space-y-3 text-sm">
              <h3 class="font-bold text-lg mb-3">Symbols</h3>
              <ul class="list-none space-y-2.5">
                {top_symbols.symbols.map((symbol) => (
                  <li class="block">
                    <a
                      href={symbol.href}
                      title={symbol.name}
                      class="flex items-center gap-2"
                    >
                      <DocNodeKindIcon kinds={symbol.kind} />
                      <span
                        class={`block w-full overflow-hidden whitespace-nowrap text-ellipsis -my-0.5 -ml-1 py-0.5 pl-1 rounded hover:bg-${
                          symbol.kind[0]?.kind
                        }/15 hover:bg-${symbol.kind[0]?.kind}Dark/15`}
                      >
                        {symbol.name}
                      </span>
                    </a>
                  </li>
                ))}
              </ul>
              {top_symbols.total_symbols > 5 && (
                <a
                  class="flex items-center gap-0.5 hover:underline"
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
            <nav class="max-sm:hidden text-sm space-y-3">
              <h3 class="font-bold text-lg mb-3">Document Navigation</h3>
              <ul>
                {renderToC(
                  document_navigation,
                  Math.min(...document_navigation.map((entry) => entry.level)),
                  0,
                )[0]}
              </ul>
            </nav>
          )}
        </div>
      </div>
    </div>
  );
}

function renderToC(
  items: ToCEntry[],
  currentLevel: number,
  startIdx: number,
  isRoot = true,
): [VNode[], number] {
  const result: VNode[] = [];
  let i = startIdx;

  while (i < items.length && items[i].level >= currentLevel) {
    const entry = items[i];

    if (entry.level === currentLevel) {
      const [children, nextIdx] =
        i + 1 < items.length && items[i + 1].level > currentLevel
          ? renderToC(items, items[i + 1].level, i + 1, false)
          : [[], i + 1];
      const hasChildren = children.length > 0;

      result.push(
        <li
          key={entry.anchor}
          class={isRoot
            ? `mx-3 ${hasChildren ? "mt-0" : "mt-2"} !pb-0`
            : "!mt-1"}
        >
          <a
            href={`#${entry.anchor}`}
            title={entry.content}
            class={`hover:underline block overflow-hidden whitespace-nowrap text-ellipsis ${
              !isRoot ? "p-1 hover:text-black dark:hover:text-white" : ""
            }`}
          >
            {entry.content}
          </a>
          {hasChildren && (
            <ul class="ml-3.5 space-y-2 text-[0.8rem] leading-none text-jsr-gray-600 dark:text-jsr-gray-200">
              {children}
            </ul>
          )}
        </li>,
      );
      i = nextIdx;
    } else {
      break;
    }
  }

  return [result, i];
}
