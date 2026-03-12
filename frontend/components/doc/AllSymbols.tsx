// Copyright 2024 the JSR authors. All rights reserved. MIT license.
import type { AllSymbolsItemCtx } from "@deno/doc/html-types";
import { ModuleDoc } from "./ModuleDoc.tsx";

export function AllSymbols({ items }: { items: AllSymbolsItemCtx[] }) {
  return (
    <div class="space-y-6 max-w-prose">
      {items.map((item) => (
        <div>
          <a href={item.href} class="link text-xl font-bold">{item.name}</a>

          <div class="ml-4 mt-2">
            <ModuleDoc content={item.module_doc} />
          </div>
        </div>
      ))}
    </div>
  );
}
