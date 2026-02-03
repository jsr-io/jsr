// Copyright 2024 the JSR authors. All rights reserved. MIT license.

export function Deprecated(
  { message }: { message?: string | null },
) {
  if (message === undefined || message === null) {
    return null;
  }

  return (
    <div class="deprecated">
      <div>
        <span>Deprecated</span>
      </div>
      {message !== "" && (
        <div
          // jsdoc rendering
          // deno-lint-ignore react-no-danger
          dangerouslySetInnerHTML={{ __html: message }} />
      )}
    </div>
  );
}
