// Copyright 2024 the JSR authors. All rights reserved. MIT license.
import type { DocEntryCtx } from "../../../new_html_types.d.ts";
import { Anchor } from "./Anchor.tsx";
import { Tag } from "./Tag.tsx";
import { SourceButton } from "./SourceButton.tsx";
import { getDiffColor } from "./mod.ts";

export function DocEntry(
  { entry }: { entry: DocEntryCtx },
) {
  const {
    name,
    name_href,
    anchor,
    tags,
    content,
    source_href,
    js_doc,
    diff_status,
    old_name,
    old_content,
    js_doc_changed,
  } = entry;

  const renamedOldName = diff_status?.kind === "renamed"
    ? (diff_status as { kind: "renamed"; old_name: string }).old_name
    : undefined;
  const effectiveOldName = old_name ?? renamedOldName;
  const signatureChanged = effectiveOldName !== undefined ||
    old_content !== undefined;

  const diffBg = getDiffColor(diff_status, false);

  return (
    <div
      class={`space-y-2 max-md:-pl-1 max-md:-ml-1 py-1 -mb-1 px-2 -mx-2 ${
        name ? "anchorable" : ""
      } group/sourceable relative diff-mobile-skip-round ${diffBg}`}
      id={anchor.id}
    >
      <div class="flex justify-between items-start md:text-base">
        <div class="break-words">
          {tags && tags.length > 0 && (
              <div class="space-x-1 mb-1">
                {tags.map((tag, i) => <Tag key={i} tag={tag} />)}
              </div>
            )}

          {signatureChanged && (
            <div
              class={`diff-removed px-1 py-0.5 mb-0.5`}
            >
              <code>
                {(effectiveOldName ?? name) && (
                  <span
                    class="font-bold font-lg"
                    // deno-lint-ignore react-no-danger
                    dangerouslySetInnerHTML={{
                      __html: effectiveOldName ?? name!,
                    }}
                  />
                )}
                <span
                  class="font-medium text-stone-500 dark:text-stone-200"
                  // deno-lint-ignore react-no-danger
                  dangerouslySetInnerHTML={{ __html: old_content ?? content }}
                />
              </code>
            </div>
          )}

          <code
            class={signatureChanged
              ? `diff-added px-1 py-0.5 block`
              : ""}
          >
            {name && anchor && <Anchor anchor={anchor} />}
            {name_href
              ? (
                <a
                  class="font-bold font-lg link"
                  href={name_href}
                  // may include type defs which are generated with spans (for ie default parameters)
                  // deno-lint-ignore react-no-danger
                  dangerouslySetInnerHTML={{ __html: name! }}
                />
              )
              : name && (
                <span
                  class="font-bold font-lg"
                  // may include type defs which are generated with spans (for ie default parameters)
                  // deno-lint-ignore react-no-danger
                  dangerouslySetInnerHTML={{ __html: name }}
                />
              )}
            <span
              class="font-medium text-stone-500 dark:text-stone-200"
              // includes type defs which are generated with spans
              // deno-lint-ignore react-no-danger
              dangerouslySetInnerHTML={{ __html: content }}
            />
          </code>
        </div>

        {source_href && <SourceButton href={source_href} />}
      </div>

      {js_doc && (
        <div
          class={`max-w-[75ch]${
            js_doc_changed
              ? " border-l-2 border-yellow-400 dark:border-yellow-600 pl-2"
              : ""
          }`}
          // jsdoc rendering
          // deno-lint-ignore react-no-danger
          dangerouslySetInnerHTML={{ __html: js_doc }}
        />
      )}
    </div>
  );
}
