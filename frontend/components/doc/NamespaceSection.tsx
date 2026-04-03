// Copyright 2024 the JSR authors. All rights reserved. MIT license.
import type { NamespaceNodeCtx } from "@deno/doc/html-types";
import { DocNodeKindIcon } from "./DocNodeKindIcon.tsx";
import { getDiffColor } from "./mod.ts";

export function NamespaceSection({ items }: { items: NamespaceNodeCtx[] }) {
  return (
    <div class="space-y-3 mt-6! max-w-prose">
      {items.map((item) => {
        const renamedOldName = item.diff_status?.kind === "renamed"
          ? item.diff_status.old_name
          : undefined;

        return (
          <div
            id={item.anchor.id}
            class={`md:min-h-16 lg:pr-4 max-md:-pl-1 max-md:-ml-1 py-1 px-2 -mt-1 -mx-2 ${
              item.deprecated
                ? "opacity-60 line-through decoration-2 decoration-stone-500/70 text-stone-500 dark:text-stone-400"
                : ""
            } diff-mobile-skip-round ${getDiffColor(item.diff_status, false)}`}
            aria-label={item.deprecated ? "deprecated" : undefined}
          >
            <div
              class={`flex gap-x-2.5 pl-1 -ml-1 pr-1.5 -mr-1.5 px-1 -mx-1 ${
                item.diff_status?.kind === "modified" ? "diff-modified" : ""
              }`}
            >
              <DocNodeKindIcon
                kinds={item.doc_node_kind_ctx}
                class="w-4 flex-col justify-start! gap-1 mt-1 [&>*+*]:ml-0 [&>*+*]:-mt-0.5"
              />

              <div class="space-y-2">
                <div class="block font-mono">
                  <a
                    href={item.href}
                    title={item.name}
                    class={`highlightable leading-none break-all font-medium underline underline-offset-2`}
                  >
                    {renamedOldName && (
                      <span class="diff-removed diff-inline ml-3">
                        {renamedOldName}
                      </span>
                    )}
                    <span
                      class={renamedOldName ? "diff-added diff-inline" : ""}
                    >
                      {item.name}
                    </span>
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

                <div>
                  {item.docs
                    ? (
                      <span
                        class="highlightable text-sm leading-5"
                        // jsdoc rendering
                        // deno-lint-ignore react-no-danger
                        dangerouslySetInnerHTML={{ __html: item.docs }}
                      />
                    )
                    : (
                      <span class="text-xs leading-none italic text-stone-600 dark:text-stone-400">
                        No documentation available
                      </span>
                    )}
                </div>
              </div>
            </div>

            {item.subitems && item.subitems.length > 0 && (
              <ul class="space-y-2.5 text-sm mt-3 ml-8.5">
                {item.subitems.map((subitem, i) => {
                  const renamedOldName = subitem.diff_status?.kind === "renamed"
                    ? subitem.diff_status.old_name
                    : undefined;

                  return (
                    <li
                      key={i}
                      class={`px-1.5 -mx-1.5 ${
                        getDiffColor(subitem.diff_status, true)
                      }`}
                    >
                      <div class="block font-mono">
                        <a
                          href={subitem.href}
                          title={subitem.title}
                          class="highlightable underline underline-offset-2"
                        >
                          {renamedOldName &&
                            (
                              <span class="diff-removed diff-inline">
                                {renamedOldName}
                              </span>
                            )}
                          <span
                            class={renamedOldName
                              ? "diff-added diff-inline"
                              : ""}
                          >
                            {subitem.title}
                          </span>
                        </a>
                        {subitem.ty && (
                          <>
                            <span
                              class="font-light opacity-85 dark:opacity-75"
                              // jsdoc rendering
                              // deno-lint-ignore react-no-danger
                              dangerouslySetInnerHTML={{
                                __html: subitem.ty.ty,
                              }}
                            />
                            {subitem.ty.info && (
                              <div class="italic text-xs ml-2 text-stone-600 dark:text-stone-400">
                                {subitem.ty.info}
                              </div>
                            )}
                          </>
                        )}
                      </div>

                      <div class="mt-1.5">
                        {subitem.docs
                          ? (
                            <span
                              class="highlightable leading-5"
                              // jsdoc rendering
                              // deno-lint-ignore react-no-danger
                              dangerouslySetInnerHTML={{
                                __html: subitem.docs,
                              }}
                            />
                          )
                          : (
                            <span class="text-xs leading-none italic text-stone-600 dark:text-stone-400">
                              No documentation available
                            </span>
                          )}
                      </div>
                    </li>
                  );
                })}
              </ul>
            )}
          </div>
        );
      })}
    </div>
  );
}
