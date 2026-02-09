// Copyright 2024 the JSR authors. All rights reserved. MIT license.
import type { BreadcrumbCtx, BreadcrumbsCtx } from "@deno/doc/html-types";
import TbChevronRight from "tb-icons/TbChevronRight";

function BreadcrumbItem(
  { part, isFirst, isLast }: {
    part: BreadcrumbCtx;
    isFirst: boolean;
    isLast: boolean;
  },
) {
  return (
    <>
      <li
        class={`inline ${
          isFirst
            ? "text-2xl leading-none font-bold"
            : "text-lg lg:text-xl leading-[0.9em]"
        }`}
      >
        {isLast ? part.name : (
          <a href={part.href} class="link">
            {part.name}
          </a>
        )}
      </li>

      {!isLast && (
        part.is_symbol
          ? <span>.</span>
          : (
            <span class="text-black dark:text-white">
              <TbChevronRight class="size-4" />
            </span>
          )
      )}
    </>
  );
}

export function Breadcrumbs(
  { breadcrumbs: { parts } }: { breadcrumbs: BreadcrumbsCtx },
) {
  const symbolStartIndex = parts.findIndex((part) => part.is_first_symbol);
  const hasSymbols = symbolStartIndex !== -1;

  const preParts = hasSymbols ? parts.slice(0, symbolStartIndex) : parts;
  const symbolParts = hasSymbols ? parts.slice(symbolStartIndex) : [];

  return (
    <ul class="break-all inline-flex flex-wrap gap-1 items-center">
      {preParts.map((part, index) => (
        <BreadcrumbItem
          part={part}
          isFirst={index === 0}
          isLast={!hasSymbols && index === preParts.length - 1}
        />
      ))}

      {hasSymbols && (
        <ul>
          {symbolParts.map((part, index) => (
            <BreadcrumbItem
              part={part}
              isFirst={false}
              isLast={index === symbolParts.length - 1}
            />
          ))}
        </ul>
      )}
    </ul>
  );
}
