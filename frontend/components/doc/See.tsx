// Copyright 2024 the JSR authors. All rights reserved. MIT license.

export interface SeeProps {
  items: string[];
}

export function See({ items }: SeeProps) {
  return (
    <ul class="see">
      {items.map((item) => (
        <li
          // jsdoc rendering
          // deno-lint-ignore react-no-danger
          dangerouslySetInnerHTML={{ __html: item }} />
      ))}
    </ul>
  );
}
