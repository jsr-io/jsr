// Copyright 2024 the JSR authors. All rights reserved. MIT license.
import type { NamespaceNodeCtx } from "../../../new_html_types.d.ts";
import { DocNodeKindIcon } from "./DocNodeKindIcon.tsx";
import { getDiffColor } from "./mod.ts";

export function NamespaceSection({ items }: { items: NamespaceNodeCtx[] }) {
  return (
    <div class="space-y-2 !mt-6 max-w-prose">
      {items.map((item) => {
        const diffBg = getDiffColor(item.diff_status, false);

        const renamedOldName = item.diff_status?.kind === "renamed"
          ? (item.diff_status as { kind: "renamed"; old_name: string }).old_name
          : undefined;

        return (
          <div
            id={item.id}
            class={`flex gap-x-2.5 md:min-h-[4rem] lg:pr-4 rounded transition duration-125 py-1 px-2 -my-1 -mx-2 ${item.deprecated ? "opacity-60" : ""} ${diffBg}`}
            aria-label={item.deprecated ? "deprecated" : undefined}
          >
            <DocNodeKindIcon
              kinds={item.doc_node_kind_ctx}
              class="w-auto flex-col !justify-start gap-1 mt-1 [&>*+*]:ml-0 [&>*+*]:-mt-0.5"
            />

            <div
              class={`w-0 flex-1 ${
                item.deprecated
                  ? "line-through decoration-2 decoration-stone-500/70 text-stone-500 dark:text-stone-400"
                  : ""
              }`}
            >
              <div class="block font-mono">
                {renamedOldName && (
                  <>
                    <span
                      class={`highlightable leading-none break-all font-medium diff-removed rounded px-0.5`}
                    >
                      {renamedOldName}
                    </span>
                    <span class="mx-1 text-stone-400">{"\u2192"}</span>
                  </>
                )}
                <a
                  href={item.href}
                  title={item.name}
                  class={`highlightable leading-none break-all font-medium underline underline-offset-2${
                    renamedOldName
                      ? ` diff-added rounded px-0.5`
                      : ""
                  }`}
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
                  {item.subitems.map((subitem) => {
                    const subDiffBg = getDiffColor(subitem.diff_status, true);

                    return (
                    <li
                      class={`rounded px-1 -mx-1 ${subDiffBg}`}
                    >
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

                      <div class="mt-2 leading-5">
                        {subitem.docs
                          ? (
                            <span
                              class="highlightable"
                              // jsdoc rendering
                              // deno-lint-ignore react-no-danger
                              dangerouslySetInnerHTML={{
                                __html: subitem.docs,
                              }}
                            />
                          )
                          : (
                            <span class="italic text-stone-600 dark:text-stone-400">
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
          </div>
        );
      })}
    </div>
  );
}
