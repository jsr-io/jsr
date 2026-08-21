// Copyright 2024 the JSR authors. All rights reserved. MIT license.

export interface SeeProps {
  items: string[];
}

export function See({ items }: SeeProps) {
  return (
    <ul class="list-disc list-inside">
      {items.map((item) => (
        <li
          class="*:inline-block"
          // jsdoc rendering
          // deno-lint-ignore react-no-danger
          dangerouslySetInnerHTML={{ __html: item }}
        />
      ))}
    </ul>
  );
}
