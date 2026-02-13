// Copyright 2024 the JSR authors. All rights reserved. MIT license.
import type { BreadcrumbCtx, BreadcrumbsCtx } from "@deno/doc/html-types";
import TbChevronRight from "tb-icons/TbChevronRight";
import DocBreadcrumbsSwitcher from "../../islands/DocBreadcrumbsSwitcher.tsx";

function BreadcrumbItem(
  { part, isFirst, isLast }: {
    part: BreadcrumbCtx;
    isFirst?: boolean;
    isLast?: boolean;
  },
) {
  return (
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
  );
}

export function Breadcrumbs(
  { breadcrumbs }: { breadcrumbs: BreadcrumbsCtx },
) {
  return (
    <ul class="break-all inline-flex flex-wrap gap-1 items-center">
      <BreadcrumbItem part={breadcrumbs.root} isFirst />

      <span class="text-black dark:text-white">
        <TbChevronRight class="size-4" />
      </span>

      <li class="inline text-lg lg:text-xl leading-[0.9em]">
        <DocBreadcrumbsSwitcher
          current={breadcrumbs.current_entrypoint!} // we know it's present, since we never render the breadcrumbs on the index page
          entrypoints={breadcrumbs.entrypoints}
          hasSymbol={breadcrumbs.symbol.length > 0}
        />
      </li>

      {breadcrumbs.symbol.length > 0 && (
        <>
          <span class="text-black dark:text-white">
            <TbChevronRight class="size-4" />
          </span>

          {breadcrumbs.symbol.map((part, i) => (
            <>
              <BreadcrumbItem
                part={part}
                isLast={i === breadcrumbs.symbol.length - 1}
              />
              {i !== (breadcrumbs.symbol.length - 1) && <span>.</span>}
            </>
          ))}
        </>
      )}
    </ul>
  );
}
