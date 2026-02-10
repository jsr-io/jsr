// Copyright 2024 the JSR authors. All rights reserved. MIT license.
import { asset } from "fresh/runtime";
import type { BreadcrumbCtx } from "@deno/doc/html-types";
import TbChevronRight from "tb-icons/TbChevronRight";
import { useComputed, useSignal, useSignalEffect } from "@preact/signals";
import { useEffect } from "preact/hooks";

export default function DocBreadcrumbsSwitcher(
  { current, entrypoints, hasSymbol }: {
    current: BreadcrumbCtx;
    entrypoints: BreadcrumbCtx[];
    hasSymbol: boolean;
  },
) {
  return (
    <details class="block max-w-64 py-1.5 px-2 relative">
      <summary class="flex gap-1 items-center select-none cursor-pointer">
        {hasSymbol
          ? (
            <a href={current.href} class="block link leading-none">
              {current.name}
            </a>
          )
          : current.name}

        <TbChevronRight class="rotate-90 size-4" />
      </summary>

      <ul class="absolute max-h-[20em] overflow-y-scroll text-base max-md:inset-x-0 mt-1.5 p-2 block z-30 md:w-max bg-white md:rounded border max-md:border-x-0 border-jsr-cyan-200 dark:border-jsr-cyan-800 dark:bg-jsr-gray-950">
        {entrypoints.map((entrypoint) => (
          <li
            class={`cursor-pointer select-none px-2 py-1 leading-normal rounded-sm hover:bg-gray-50 dark:hover:bg-gray-800 ${
              entrypoint.name === current.name
                ? "font-semibold bg-gray-50 dark:bg-gray-800"
                : ""
            }`}
          >
            <a href={entrypoint.href} class="block w-full">{entrypoint.name}</a>
          </li>
        ))}
      </ul>
    </details>
  );
}
