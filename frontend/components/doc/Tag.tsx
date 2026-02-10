// Copyright 2024 the JSR authors. All rights reserved. MIT license.
import type { Tag as TagType } from "@deno/doc/html-types";

function titleCase(str: string): string {
  return str.charAt(0).toUpperCase() + str.slice(1);
}

export function Tag({ tag, large }: { tag: TagType; large?: boolean }) {
  const sizeClasses = large ? "font-bold py-2 px-3" : "text-sm py-1 px-2";

  const renderContent = () => {
    if (large) {
      if ("value" in tag && tag.value) {
        if (tag.kind === "permissions") {
          const permissions = tag.value as string[];
          return (
            <span class="space-x-2">
              {permissions.map((perm, index) => (
                <>
                  <span>{perm}</span>
                  {index < permissions.length - 1 && (
                    <div class="inline border-l-2 border-stone-300 dark:border-gray-700" />
                  )}
                </>
              ))}
            </span>
          );
        }
        return titleCase(tag.value as string);
      }
      return titleCase(tag.kind);
    }

    if ("value" in tag && tag.value) {
      return tag.value as string;
    }
    return tag.kind;
  };

  return (
    <div
      class={`text-${tag.kind} border border-${tag.kind}/50 bg-${tag.kind}/5 inline-flex items-center gap-0.5 *:flex-none rounded-md leading-none ${sizeClasses}`}
    >
      {renderContent()}
    </div>
  );
}
