// Copyright 2024 the JSR authors. All rights reserved. MIT license.
import type { SymbolContentCtx } from "@deno/doc/html-types";
import { Section } from "./Section.tsx";

export function SymbolContent(
  { content: { id, docs, sections } }: { content: SymbolContentCtx },
) {
  return (
    <div class="space-y-7" id={id}>
      {docs && (
        <div
          // jsdoc rendering
          // deno-lint-ignore react-no-danger
          dangerouslySetInnerHTML={{ __html: docs }}
        />
      )}
      {sections.map((section, i) => <Section key={i} section={section} />)}
    </div>
  );
}
