// Copyright 2024 the JSR authors. All rights reserved. MIT license.
import type { ModuleDocCtx } from "@deno/doc/html-types";
import { Deprecated } from "./Deprecated.tsx";
import { SymbolContent } from "./SymbolContent.tsx";

export function ModuleDoc({ content }: { content: ModuleDocCtx }) {
  return (
    <section>
      <div class="space-y-2 flex-1">
        <Deprecated message={content.deprecated} />
        <SymbolContent content={content.sections} />
      </div>
    </section>
  );
}
