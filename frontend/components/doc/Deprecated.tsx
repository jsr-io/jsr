// Copyright 2024 the JSR authors. All rights reserved. MIT license.

export function Deprecated(
  { message }: { message?: string | null },
) {
  if (message === undefined || message === null) {
    return null;
  }

  return (
    <div class="deprecated">
      <div class="py-1 text-red-500 flex gap-1 items-center dark:text-red-400">
        <span class="font-semibold leading-6">Deprecated</span>
      </div>
      {message !== "" && (
        <div
          class="ml-1 pl-2 border-l-4 border-red-300 dark:border-red-600"
          // jsdoc rendering
          // deno-lint-ignore react-no-danger
          dangerouslySetInnerHTML={{ __html: message }}
        />
      )}
    </div>
  );
}
