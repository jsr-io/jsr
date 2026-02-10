// Copyright 2024 the JSR authors. All rights reserved. MIT license.
import type { CategoriesPanelCtx } from "@deno/doc/html-types";
import TbChevronRight from "tb-icons/TbChevronRight";

export function CategoryPanel(
  { panel: { categories, all_symbols_href, total_symbols } }: {
    panel: CategoriesPanelCtx;
  },
) {
  if (!categories?.length) {
    return null;
  }

  return (
    <div id="categoryPanel">
      <ul>
        {categories.map((category, i) => (
          <li key={i} class={category.active ? "active" : undefined}>
            <a href={category.href} title={category.name}>
              {category.name}
            </a>
          </li>
        ))}
        <li>
          <a class="!flex items-center gap-0.5" href={all_symbols_href}>
            <span class="leading-none">view all {total_symbols} symbols</span>
            <TbChevronRight class="size-4" />
          </a>
        </li>
      </ul>
    </div>
  );
}
