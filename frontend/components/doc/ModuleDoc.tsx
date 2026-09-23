// Copyright 2024 the JSR authors. All rights reserved. MIT license.
import type { ModuleDocCtx } from "@deno/doc/html-types";
import { Deprecated } from "./Deprecated.tsx";
import { SymbolContent } from "./SymbolContent.tsx";

export function ModuleDoc({ content }: { content: ModuleDocCtx }) {
  const symbolsTrimmed = content.symbols_trimmed;

  return (
    <section>
      <div class="space-y-2 flex-1">
        <Deprecated message={content.deprecated} />
        {symbolsTrimmed && (
          <div class="italic text-sm">
            Some deeply nested symbols were omitted from this overview; they are
            listed on the page of their containing namespace.
          </div>
        )}
        <SymbolContent content={content.sections} />
      </div>
    </section>
  );
}
