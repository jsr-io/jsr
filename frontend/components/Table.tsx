// Copyright 2024 the JSR authors. All rights reserved. MIT license.

import { ComponentChild, ComponentChildren } from "preact";
import {
  TbChevronLeft,
  TbChevronRight,
  TbSortAscending,
  TbSortDescending,
} from "tb-icons";
import { PaginationData } from "../util.ts";

interface TableProps {
  columns: ColumnProps[];
  children: ComponentChild[];
  pagination?: PaginationData;
  sortBy?: string;
  class?: string;
  currentUrl: URL;
}

interface ColumnProps {
  title: ComponentChildren;
  align?: "left" | "right";
  class?: string;
  fieldName?: string;
}

export function Table(
  { columns, children, pagination, currentUrl, class: class_, sortBy: sortBy_ }:
    TableProps,
) {
  let sortBy = sortBy_;
  let desc = true;
  if (sortBy_?.startsWith("!")) {
    sortBy = sortBy_.slice(1);
    desc = false;
  }

  return (
    <div
      class={`-mx-4 md:mx-0 ring-1 ring-jsr-cyan-100 dark:ring-jsr-cyan-900 sm:rounded overflow-hidden ${
        class_ ?? ""
      }`}
    >
      <div class="overflow-x-auto">
        <table class="w-full">
          <thead class="bg-jsr-cyan-50 dark:bg-jsr-cyan-950 border-b border-jsr-cyan-100 dark:border-jsr-cyan-900">
            <TableRow class="children:font-semibold">
              {columns.map(({ align, class: _class, title, fieldName }) => {
                let icon;

                if (fieldName) {
                  if (sortBy === fieldName) {
                    if (desc) {
                      icon = (
                        <TbSortDescending class="size-5" aria-hidden="true" />
                      );
                    } else {
                      icon = (
                        <TbSortAscending class="size-5" aria-hidden="true" />
                      );
                    }
                  } else {
                    icon = (
                      <TbSortDescending
                        class="size-5 text-gray-400 group-hover:text-inherit"
                        aria-hidden="true"
                      />
                    );
                  }
                }

                const url = new URL(currentUrl);
                if (fieldName) {
                  url.searchParams.set(
                    "sortBy",
                    (sortBy === fieldName && desc)
                      ? `!${fieldName}`
                      : fieldName,
                  );
                }

                return (
                  <th
                    scope="col"
                    aria-sort={sortBy === fieldName
                      ? (desc ? "descending" : "ascending")
                      : undefined}
                    class={`py-2.5 px-3 first:pl-4 first:sm:pl-6 last:pr-4 last:sm:pr-6 whitespace-nowrap text-sm text-primary ${
                      _class ?? ""
                    }`}
                  >
                    {fieldName
                      ? (
                        <a
                          class={`flex items-center gap-2.5 group select-none ${
                            align === "right" ? "justify-end" : ""
                          }`}
                          href={url.pathname + url.search}
                        >
                          {title}
                          {icon}
                        </a>
                      )
                      : (
                        <div
                          class={`flex items-center gap-2.5 group select-none ${
                            align === "right" ? "justify-end" : ""
                          }`}
                        >
                          {title}
                          {icon}
                        </div>
                      )}
                  </th>
                );
              })}
            </TableRow>
          </thead>
          <tbody class="divide-y divide-jsr-cyan-300/30 dark:divide-jsr-cyan-900 bg-white dark:bg-jsr-gray-950">
            {children}
          </tbody>
          {pagination && (
            <tfoot class="bg-white dark:bg-jsr-gray-950 border-t border-jsr-cyan-100 dark:border-jsr-cyan-900">
              <tr>
                <td colspan={columns.length} class="py-3 px-4 sm:px-6">
                  <div class="flex justify-end">
                    <Pagination
                      pagination={pagination}
                      itemsCount={children.length}
                      currentUrl={currentUrl}
                    />
                  </div>
                </td>
              </tr>
            </tfoot>
          )}
        </table>
      </div>
    </div>
  );
}

function Pagination(
  { pagination, itemsCount, currentUrl }: {
    pagination: PaginationData;
    itemsCount: number;
    currentUrl: URL;
  },
) {
  const hasPrevious = pagination.page > 1;
  const hasNext = pagination.limit * pagination.page < pagination.total;

  const prevURL = new URL(currentUrl);
  prevURL.searchParams.set("page", (pagination.page - 1).toString());
  const nextURL = new URL(currentUrl);
  nextURL.searchParams.set("page", (pagination.page + 1).toString());

  return (
    <nav
      aria-label="Table pagination"
      class="flex items-center gap-3 text-secondary"
    >
      {hasPrevious && (
        <a
          href={prevURL.pathname + prevURL.search}
          class="hover:text-black dark:hover:text-white hover:bg-jsr-cyan-100 dark:hover:bg-jsr-gray-900 p-1 -m-1 rounded-full"
          aria-label="Go to previous page"
          title="Previous page"
        >
          <TbChevronLeft class="size-5" aria-hidden="true" />
        </a>
      )}
      <div class="text-sm text-secondary">
        Showing items {(pagination.page * pagination.limit) - pagination.limit +
          (Math.min(itemsCount, 1))}
        -
        {(pagination.page * pagination.limit) - pagination.limit +
          ((itemsCount < pagination.limit) ? itemsCount : pagination.limit)} of
        {" "}
        {pagination.total}
      </div>
      {hasNext && (
        <a
          href={nextURL.pathname + nextURL.search}
          class="hover:text-black dark:hover:text-white hover:bg-jsr-gray-100 dark:hover:bg-jsr-gray-900 p-1 -m-1 rounded-full"
          aria-label="Go to next page"
          title="Next page"
        >
          <TbChevronRight class="size-5" aria-hidden="true" />
        </a>
      )}
    </nav>
  );
}

interface TableRowProps {
  children: ComponentChildren;
  class?: string;
}

export function TableRow({
  children,
  class: _class,
}: TableRowProps) {
  return (
    <tr
      class={`children:whitespace-nowrap children:text-sm children:text-jsr-gray-700 dark:children:text-gray-300 ${
        _class ?? ""
      }`}
    >
      {children}
    </tr>
  );
}

interface TableDataProps {
  children: ComponentChildren;
  title?: string;
  class?: string;
  flex?: boolean;
  align?: "left" | "right";
}

export function TableData(
  {
    children,
    class: _class,
    align,
    title,
    flex,
  }: TableDataProps,
) {
  return (
    <td
      class={`py-3 px-3 first:pl-4 first:sm:pl-6 last:pr-4 last:sm:pr-6 whitespace-nowrap text-sm text-primary ${
        _class ?? ""
      } ${align === "right" ? "text-right" : "text-left"}`}
      title={title}
    >
      {flex
        ? <div class="flex items-center gap-2.5">{children}</div>
        : children}
    </td>
  );
}
