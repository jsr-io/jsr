// Copyright 2024 the JSR authors. All rights reserved. MIT license.

import { ComponentChild, ComponentChildren } from "preact";
import { Head } from "$fresh/runtime.ts";
import { ChevronLeft } from "./icons/ChevronLeft.tsx";
import { ChevronRight } from "./icons/ChevronRight.tsx";
import { PaginationData } from "../util.ts";

interface TableProps {
  columns: ColumnProps[];
  children: ComponentChild[];
  pagination?: PaginationData;
  class?: string;
  currentUrl: URL;
}

interface ColumnProps {
  title: ComponentChildren;
  align?: "left" | "right";
  class?: string;
}

export function Table(
  { columns, children, pagination, currentUrl, class: class_ }: TableProps,
) {
  return (
    <>
      <div
        class={`-mx-4 md:mx-0 ring-1 ring-jsr-cyan-950 sm:rounded overflow-hidden ${
          class_ ?? ""
        }`}
      >
        <div class="overflow-x-auto">
          <table class="w-full divide-y divide-jsr-cyan-900/10">
            <thead class="bg-jsr-cyan-50">
              <TableRow class="children:font-semibold">
                {columns.map((column, i) => (
                  <TableHead
                    class={column.class}
                    align={column.align}
                  >
                    {column.title}
                  </TableHead>
                ))}
              </TableRow>
            </thead>
            <tbody class="divide-y divide-cyan-900/10 bg-white">
              {children}
            </tbody>
          </table>
        </div>
      </div>
      <div class="py-3 sm:px-6 flex justify-end items-center gap-6">
        {pagination && (
          <Pagination
            pagination={pagination}
            itemsCount={children.length}
            currentUrl={currentUrl}
          />
        )}
      </div>
    </>
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
    <div class="flex items-center gap-3 text-gray-700">
      <Head>
        {hasPrevious && (
          <link rel="prev" href={prevURL.pathname + prevURL.search} />
        )}
        {hasNext && (
          <link
            rel="next"
            href={nextURL.pathname + nextURL.search}
          />
        )}
      </Head>

      {hasPrevious && (
        <a
          href={prevURL.pathname + prevURL.search}
          class="hover:text-black hover:bg-cyan-100 p-1 -m-1 rounded-full"
          title="Previous page"
        >
          <ChevronLeft />
        </a>
      )}
      <div class="text-sm text-gray-600">
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
          class="hover:text-black hover:bg-gray-100 p-1 -m-1 rounded-full"
          title="Next page"
        >
          <ChevronRight />
        </a>
      )}
    </div>
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
      class={`children:whitespace-nowrap children:text-sm children:text-gray-700 ${
        _class ?? ""
      }`}
    >
      {children}
    </tr>
  );
}

interface TableHeadProps {
  children: ComponentChildren;
  class?: string;
  align?: "left" | "right";
}

export function TableHead({
  children,
  class: _class,
  align,
}: TableHeadProps) {
  return (
    <th
      class={`py-4 px-3 first:pl-4 first:sm:pl-6 last:pr-4 last:sm:pr-6 whitespace-nowrap text-sm text-gray-900 ${
        _class ?? ""
      } ${align === "right" ? "text-right" : "text-left"}`}
    >
      {children}
    </th>
  );
}

interface TableDataProps {
  children: ComponentChildren;
  title?: string;
  class?: string;
  align?: "left" | "right";
}

export function TableData(
  {
    children,
    class: _class,
    align,
    title,
  }: TableDataProps,
) {
  return (
    <td
      class={`py-4 px-3 first:pl-4 first:sm:pl-6 last:pr-4 last:sm:pr-6 whitespace-nowrap text-sm text-gray-900 ${
        _class ?? ""
      } ${align === "right" ? "text-right" : "text-left"}`}
      title={title}
    >
      {children}
    </td>
  );
}
