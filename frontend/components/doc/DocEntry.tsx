// Copyright 2024 the JSR authors. All rights reserved. MIT license.
import type { DocEntryCtx } from "@deno/doc/html-types";
import { Anchor } from "./Anchor.tsx";
import { Tag } from "./Tag.tsx";
import { SourceButton } from "./SourceButton.tsx";
import { getDiffColor } from "./mod.ts";

export function DocEntry(
  { entry }: { entry: DocEntryCtx },
) {
  const {
    name_prefix,
    name,
    name_href,
    anchor,
    tags,
    content,
    source_href,
    js_doc,
    diff_status,
    old_content,
  } = entry;

  const renamedOldName = diff_status?.kind === "renamed"
    ? diff_status.old_name
    : undefined;

  return (
    <div
      class={`space-y-2 max-md:-pl-1 max-md:-ml-1 py-1 px-2 -mx-2 ${
        name ? "anchorable" : ""
      } group/sourceable relative diff-mobile-skip-round ${
        getDiffColor(diff_status, false)
      }`}
      id={anchor.id}
    >
      <div class="flex justify-between items-start md:text-base">
        <div class="wrap-break-word">
          {tags && tags.length > 0 && (
            <div class="space-x-1 mb-1">
              {tags.map((tag, i) => <Tag key={i} tag={tag} />)}
            </div>
          )}

          <code>
            {name && (
              <Anchor
                anchor={anchor}
                class={(diff_status && diff_status.kind !== "modified")
                  ? (renamedOldName ? "!-ml-10" : "!-ml-12")
                  : ""}
              />
            )}
            <span class="font-bold font-lg align-top">
              {name_prefix && (
                <span class="text-secondary">{name_prefix + " "}</span>
              )}

              {renamedOldName && (
                <span
                  class="diff-removed diff-inline"
                  // may include type defs which are generated with spans (for ie default parameters)
                  // deno-lint-ignore react-no-danger
                  dangerouslySetInnerHTML={{ __html: renamedOldName }}
                />
              )}
              {name_href
                ? (
                  <a
                    class={`link ${
                      renamedOldName ? "diff-added diff-inline" : ""
                    }`}
                    href={name_href}
                    // may include type defs which are generated with spans (for ie default parameters)
                    // deno-lint-ignore react-no-danger
                    dangerouslySetInnerHTML={{ __html: name! }}
                  />
                )
                : name && (
                  <span
                    class={renamedOldName ? "diff-added diff-inline" : ""}
                    // may include type defs which are generated with spans (for ie default parameters)
                    // deno-lint-ignore react-no-danger
                    dangerouslySetInnerHTML={{ __html: name }}
                  />
                )}
            </span>
            <span
              class={`font-medium text-stone-500 dark:text-stone-200 ${
                old_content ? "inline-block ml-5" : ""
              }`}
            >
              {old_content && (
                <span
                  class="block diff-removed diff-flat-bottom px-0.5"
                  // includes type defs which are generated with spans
                  // deno-lint-ignore react-no-danger
                  dangerouslySetInnerHTML={{ __html: old_content }}
                />
              )}
              <span
                class={old_content
                  ? "block diff-added diff-flat-top px-0.5"
                  : ""}
                // includes type defs which are generated with spans
                // deno-lint-ignore react-no-danger
                dangerouslySetInnerHTML={{ __html: content }}
              />
            </span>
          </code>
        </div>

        {source_href && <SourceButton href={source_href} />}
      </div>

      {js_doc && (
        <div
          class="max-w-[75ch]"
          // jsdoc rendering
          // deno-lint-ignore react-no-danger
          dangerouslySetInnerHTML={{ __html: js_doc }}
        />
      )}
    </div>
  );
}
