// Copyright 2024 the JSR authors. All rights reserved. MIT license.
import type { ComponentChildren } from "preact";
import type { RefObject } from "preact";

export function URLQuerySearch(
  { query, children, formRef }: {
    query: string;
    children?: ComponentChildren;
    formRef?: RefObject<HTMLFormElement>;
  },
) {
  return (
    <form method="GET" class="flex mt-4 flex-grow gap-4" ref={formRef}>
      {children}
      <input
        type="text"
        name="search"
        value={query}
        placeholder="Search"
        class="block w-full p-1.5 input-container input"
      />
      <button type="submit" class="button-primary">
        Search
      </button>
    </form>
  );
}
