// Copyright 2024 the JSR authors. All rights reserved. MIT license.
import type { FunctionCtx } from "@deno/doc/html-types";
import { Anchor } from "./Anchor.tsx";
import { Deprecated } from "./Deprecated.tsx";
import { SymbolContent } from "./SymbolContent.tsx";

export function Function({ functions }: FunctionCtx) {
  return (
    <div class="mt-3 space-y-8">
      {functions.map((func, index) => (
        <>
          <div class="scroll-mt-16" id={func.id}>
            <code class="anchorable text-base break-words">
              <Anchor {...func.anchor} />
              <span class="font-bold">{func.name}</span>
              <span
                class="font-medium"
                dangerouslySetInnerHTML={{ __html: func.summary }}
              />
            </code>

            <Deprecated message={func.deprecated} />

            <SymbolContent content={func.content} />
          </div>
          {index < functions.length - 1 && (
            <div class="border-b border-gray-300 max-w-[75ch] dark:border-gray-700" />
          )}
        </>
      ))}
    </div>
  );
}
