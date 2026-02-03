// Copyright 2024 the JSR authors. All rights reserved. MIT license.
import type { DocEntryCtx } from "@deno/doc/html-types";
import { Anchor } from "./Anchor.tsx";
import { Tag } from "./Tag.tsx";
import { SourceButton } from "./SourceButton.tsx";

export function DocEntry(
  { id, name, name_href, anchor, tags, content, source_href, js_doc }:
    DocEntryCtx,
) {
  return (
    <div class={name ? "anchorable docEntry" : "docEntry"} id={id}>
      <div class="docEntryHeader">
        <div>
          {tags && tags.length > 0 && (
            <div class="space-x-1 mb-1">
              {tags.map((tag) => <Tag value={tag} />)}
            </div>
          )}

          <code>
            {name && anchor && <Anchor {...anchor} />}
            {name_href
              ? (
                <a
                  class="font-bold font-lg link"
                  href={name_href}
                  dangerouslySetInnerHTML={{ __html: name ?? "" }}
                />
              )
              : name && (
                <span
                  class="font-bold font-lg"
                  dangerouslySetInnerHTML={{ __html: name }}
                />
              )}
            <span
              class="font-medium text-stone-500 dark:text-stone-200"
              dangerouslySetInnerHTML={{ __html: content }}
            />
          </code>
        </div>

        {source_href && <SourceButton href={source_href} />}
      </div>

      {js_doc && (
        <div
          class="max-w-[75ch]"
          dangerouslySetInnerHTML={{ __html: js_doc }}
        />
      )}
    </div>
  );
}
