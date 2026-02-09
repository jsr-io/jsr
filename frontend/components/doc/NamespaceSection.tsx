// Copyright 2024 the JSR authors. All rights reserved. MIT license.
import type { NamespaceNodeCtx } from "@deno/doc/html-types";
import { DocNodeKindIcon } from "./DocNodeKindIcon.tsx";

const ATagClasses =
  "underline decoration-stone-300 dark:decoration-stone-500 hover:no-underline";

export function NamespaceSection({ items }: { items: NamespaceNodeCtx[] }) {
  return (
    <div class="space-y-3 mt-4 max-w-prose">
      {items.map((item) => (
        <div
          id={item.id}
          class={`namespaceItem flex gap-x-2.5 md:min-h-[4rem] lg:pr-4 min-h-12 ${
            item.deprecated ? "opacity-60" : ""
          }`}
          aria-label={item.deprecated ? "deprecated" : undefined}
        >
          <DocNodeKindIcon
            kinds={item.doc_node_kind_ctx}
            class="w-auto flex-col !justify-start gap-1 [&>*+*]:ml-0 [&>*+*]:-mt-0.5"
          />

          <div class="namespaceItemContent w-0 flex-1">
            <a
              href={item.href}
              title={item.name}
              class={`${ATagClasses} leading-none block break-all font-medium ${
                item.deprecated
                  ? "line-through decoration-2 decoration-stone-500/70 text-stone-500 dark:text-stone-400"
                  : ""
              }`}
            >
              {item.name}
            </a>

            <div class="mt-2 text-sm leading-5 text-stone-600 dark:text-stone-400">
              {item.docs
                ? (
                  <span
                    // jsdoc rendering
                    // deno-lint-ignore react-no-danger
                    dangerouslySetInnerHTML={{ __html: item.docs }}
                  />
                )
                : <span class="italic">No documentation available</span>}
            </div>

            {item.subitems && item.subitems.length > 0 && (
              <ul class="namespaceItemContentSubItems flex flex-wrap gap-y-1 text-sm">
                {item.subitems.map((subitem, i) => (
                  <li
                    class={i !== item.subitems.length - 1
                      ? "after:content-['|'] after:mx-2 after:text-gray-300 after:select-none after:dark:text-gray-500"
                      : ""}
                  >
                    <a href={subitem.href} class={ATagClasses}>
                      {subitem.title}
                    </a>
                  </li>
                ))}
              </ul>
            )}
          </div>
        </div>
      ))}
    </div>
  );
}
