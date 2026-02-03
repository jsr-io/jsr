// Copyright 2024 the JSR authors. All rights reserved. MIT license.

export interface SeeProps {
  items: string[];
}

export function See({ items }: SeeProps) {
  return (
    <ul class="see">
      {items.map((item) => (
        <li dangerouslySetInnerHTML={{ __html: item }} />
      ))}
    </ul>
  );
}
