// Copyright 2024 the JSR authors. All rights reserved. MIT license.
import { PaginationData } from "../util.ts";
import TbChevronRight from "tb-icons/TbChevronRight";
import { ComponentChildren } from "preact";

export interface ListDisplayItem {
  href: string;
  content: ComponentChildren;
  parentClass?: string;
}

export function ListDisplay(
  { title, pagination, currentUrl, children }: {
    title?: string;
    pagination?: PaginationData;
    currentUrl?: URL;
    children: ListDisplayItem[];
  },
) {
  return (
    <div class="mt-8 ring-1 ring-jsr-cyan-100 dark:ring-jsr-gray-700 rounded overflow-hidden">
      {title &&
        (
          <div class="px-5 py-4 flex items-center justify-between border-b border-jsr-cyan-50 dark:border-jsr-gray-700 bg-jsr-gray-50 dark:bg-jsr-gray-800 leading-none">
            <span class="font-semibold">{title}</span>
            <div />
          </div>
        )}

      <ul class="divide-y dark:divide-jsr-gray-700">
        {children.map((item) => (
          <li class="border-jsr-cyan-50 dark:border-jsr-gray-700 bg-white dark:bg-jsr-gray-900">
            <a
              href={item.href}
              class={`flex items-center px-5 py-3 gap-2 hover:bg-jsr-yellow-100 dark:hover:bg-jsr-gray-800 focus:bg-jsr-yellow-100 dark:focus:bg-jsr-gray-800 focus:ring-2 ring-jsr-cyan-700 dark:ring-jsr-cyan-500 ring-inset outline-none ${
                item.parentClass ?? ""
              }`}
            >
              {item.content}

              <TbChevronRight class="text-jsr-cyan-800 dark:text-jsr-cyan-400 flex-shrink-0 size-6" />
            </a>
          </li>
        ))}
      </ul>

      {pagination && (
        <Pagination
          pagination={pagination}
          itemsCount={children.length}
          currentUrl={currentUrl!}
        />
      )}
    </div>
  );
}

function Pagination(
  { currentUrl, itemsCount, pagination }: {
    currentUrl: URL;
    itemsCount: number;
    pagination: PaginationData;
  },
) {
  const start = pagination.page * pagination.limit - pagination.limit;

  const prevURL = new URL(currentUrl);
  prevURL.searchParams.set("page", (pagination.page - 1).toString());
  const nextURL = new URL(currentUrl);
  nextURL.searchParams.set("page", (pagination.page + 1).toString());

  const hasPrevious = pagination.page > 1;
  const hasNext = pagination.limit * pagination.page < pagination.total;

  return (
    <nav
      class="flex items-center justify-between border-t border-jsr-cyan-900/10 dark:border-jsr-gray-700 bg-white dark:bg-jsr-gray-900 px-4 py-3 sm:px-6"
      aria-label="Pagination"
    >
      <div class="hidden sm:block">
        <p class="text-sm text-jsr-gray-700 dark:text-gray-300">
          {start + itemsCount === 0 ? "No results found" : (
            <>
              Showing <span class="font-semibold">{start + 1}</span> to{" "}
              <span class="font-semibold">{start + itemsCount}</span>{" "}
              results, out of{" "}
              <span class="font-semibold">{pagination.total}</span>
            </>
          )}
        </p>
      </div>
      <div class="flex flex-1 justify-between sm:justify-end">
        {hasPrevious
          ? (
            <a
              href={prevURL.pathname + prevURL.search}
              class="relative inline-flex items-center rounded-md bg-white dark:bg-jsr-gray-800 px-3 py-2 text-sm font-semibold text-jsr-gray-900 dark:text-gray-200 ring-1 ring-inset ring-jsr-gray-300 dark:ring-jsr-gray-600 hover:bg-jsr-gray-50 dark:hover:bg-jsr-gray-700 focus-visible:outline-offset-0 select-none"
            >
              Previous
            </a>
          )
          : <span />}
        {hasNext
          ? (
            <a
              href={nextURL.pathname + nextURL.search}
              class="relative ml-3 inline-flex items-center rounded-md bg-white dark:bg-jsr-gray-800 px-3 py-2 text-sm font-semibold text-jsr-gray-900 dark:text-gray-200 ring-1 ring-inset ring-jsr-gray-300 dark:ring-jsr-gray-600 hover:bg-jsr-gray-50 dark:hover:bg-jsr-gray-700 focus-visible:outline-offset-0 select-none"
            >
              Next
            </a>
          )
          : <span />}
      </div>
    </nav>
  );
}
