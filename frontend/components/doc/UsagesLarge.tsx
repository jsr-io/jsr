// Copyright 2024 the JSR authors. All rights reserved. MIT license.
import type { UsagesCtx } from "@deno/doc/html-types";

export function UsagesLarge({ usages }: UsagesCtx) {
  return (
    <div class="usageContent px-4 pt-4 pb-5 bg-stone-100 rounded border border-gray-300 dark:border-gray-700 dark:bg-gray-800 dark:text-gray-200">
      <h3>Usage in Deno</h3>
      <div dangerouslySetInnerHTML={{ __html: usages[0]?.content ?? "" }} />
    </div>
  );
}
