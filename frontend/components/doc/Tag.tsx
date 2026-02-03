// Copyright 2024 the JSR authors. All rights reserved. MIT license.
import type { Tag as TagType } from "@deno/doc/html-types";

function titleCase(str: string): string {
  return str.charAt(0).toUpperCase() + str.slice(1);
}

export interface TagProps {
  value: TagType;
  large?: boolean;
}

export function Tag({ value, large }: TagProps) {
  const sizeClasses = large ? "font-bold py-2 px-3" : "text-sm py-1 px-2";

  const renderContent = () => {
    if (large) {
      if ("value" in value && value.value) {
        if (value.kind === "permissions") {
          const permissions = value.value as string[];
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
        return titleCase(value.value as string);
      }
      return titleCase(value.kind);
    }

    if ("value" in value && value.value) {
      return value.value as string;
    }
    return value.kind;
  };

  return (
    <div
      class={`text-${value.kind} border border-${value.kind}/50 bg-${value.kind}/5 inline-flex items-center gap-0.5 *:flex-none rounded-md leading-none ${sizeClasses}`}
    >
      {renderContent()}
    </div>
  );
}
