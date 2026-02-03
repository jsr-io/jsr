// Copyright 2024 the JSR authors. All rights reserved. MIT license.
import type { NamespaceNodeCtx } from "@deno/doc/html-types";
import { DocNodeKindIcon } from "./DocNodeKindIcon.tsx";

export function NamespaceSection({ items }: { items: NamespaceNodeCtx[] }) {
  return (
    <div class="namespaceSection">
      {items.map((item) => (
        <div
          id={item.id}
          class="namespaceItem"
          aria-label={item.deprecated ? "deprecated" : undefined}
        >
          <DocNodeKindIcon kinds={item.doc_node_kind_ctx} />

          <div class="namespaceItemContent">
            <a href={item.href} title={item.name}>
              {item.name}
            </a>

            <div class="namespaceItemContentDoc">
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
              <ul class="namespaceItemContentSubItems">
                {item.subitems.map((subitem) => (
                  <li>
                    <a href={subitem.href}>{subitem.title}</a>
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
