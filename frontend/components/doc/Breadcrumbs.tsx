// Copyright 2024 the JSR authors. All rights reserved. MIT license.
import type { BreadcrumbCtx, BreadcrumbsCtx } from "@deno/doc/html-types";
import TbChevronRight from "tb-icons/TbChevronRight";

function BreadcrumbItem(
  { part, isLast }: { part: BreadcrumbCtx; isLast: boolean },
) {
  return (
    <>
      <li>
        {isLast ? part.name : (
          <a href={part.href} class="contextLink">
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
    <ul class="breadcrumbs">
      {preParts.map((part, index) => (
        <BreadcrumbItem
          part={part}
          isLast={!hasSymbols && index === preParts.length - 1}
        />
      ))}

      {hasSymbols && (
        <ul>
          {symbolParts.map((part, index) => (
            <BreadcrumbItem
              part={part}
              isLast={index === symbolParts.length - 1}
            />
          ))}
        </ul>
      )}
    </ul>
  );
}
