// Copyright 2024 the JSR authors. All rights reserved. MIT license.
import type { NamespaceNodeCtx } from "@deno/doc/html-types";
import { DocNodeKindIcon } from "./DocNodeKindIcon.tsx";

export function NamespaceSection({ items }: { items: NamespaceNodeCtx[] }) {
  return (
    <div class="space-y-2 mt-6! max-w-prose">
      {items.map((item) => (
        <div
          id={item.id}
          class={`flex gap-x-2.5 md:min-h-16 lg:pr-4 rounded transition duration-125 ${
            item.deprecated ? "opacity-60" : ""
          }`}
          aria-label={item.deprecated ? "deprecated" : undefined}
        >
          <DocNodeKindIcon
            kinds={item.doc_node_kind_ctx}
            class="w-auto flex-col justify-start! gap-1 mt-1 [&>*+*]:ml-0 [&>*+*]:-mt-0.5"
          />

          <div
            class={`w-0 flex-1 ${
              item.deprecated
                ? "line-through decoration-2 decoration-stone-500/70 text-stone-500 dark:text-stone-400"
                : ""
            }`}
          >
            <div class="block font-mono">
              <a
                href={item.href}
                title={item.name}
                class="highlightable leading-none break-all font-medium underline underline-offset-2"
              >
                {item.name}
              </a>
              {item.ty && (
                <>
                  <span
                    class="font-light opacity-85 dark:opacity-75"
                    // jsdoc rendering
                    // deno-lint-ignore react-no-danger
                    dangerouslySetInnerHTML={{ __html: item.ty.ty }}
                  />
                  {item.ty.info && (
                    <div class="italic text-xs ml-2 text-stone-600 dark:text-stone-400">
                      {item.ty.info}
                    </div>
                  )}
                </>
              )}
            </div>

            <div class="mt-2 text-sm leading-5">
              {item.docs
                ? (
                  <span
                    class="highlightable"
                    // jsdoc rendering
                    // deno-lint-ignore react-no-danger
                    dangerouslySetInnerHTML={{ __html: item.docs }}
                  />
                )
                : (
                  <span class="italic text-stone-600 dark:text-stone-400">
                    No documentation available
                  </span>
                )}
            </div>

            {item.subitems && item.subitems.length > 0 && (
              <ul class="gap-y-3 text-sm mt-3 ml-2">
                {item.subitems.map((subitem) => (
                  <li>
                    <div class="block font-mono">
                      <a
                        href={subitem.href}
                        title={subitem.title}
                        class="highlightable underline underline-offset-2"
                      >
                        {subitem.title}
                      </a>
                      {subitem.ty && (
                        <>
                          <span
                            class="font-light opacity-85 dark:opacity-75"
                            // jsdoc rendering
                            // deno-lint-ignore react-no-danger
                            dangerouslySetInnerHTML={{ __html: subitem.ty.ty }}
                          />
                          {subitem.ty.info && (
                            <div class="italic text-xs ml-2 text-stone-600 dark:text-stone-400">
                              {subitem.ty.info}
                            </div>
                          )}
                        </>
                      )}
                    </div>

                    <div class="mt-2 leading-5">
                      {subitem.docs
                        ? (
                          <span
                            class="highlightable"
                            // jsdoc rendering
                            // deno-lint-ignore react-no-danger
                            dangerouslySetInnerHTML={{ __html: subitem.docs }}
                          />
                        )
                        : (
                          <span class="italic text-stone-600 dark:text-stone-400">
                            No documentation available
                          </span>
                        )}
                    </div>
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
