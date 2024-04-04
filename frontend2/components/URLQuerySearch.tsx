// Copyright 2024 the JSR authors. All rights reserved. MIT license.
export function URLQuerySearch({ query }: { query: string }) {
  return (
    <form method="GET" class="flex mt-4">
      <input
        type="text"
        name="search"
        value={query}
        placeholder="Search"
        class="block w-full p-1.5 input-container input"
      />
      <button type="submit" class="button-primary ml-4">
        Search
      </button>
    </form>
  );
}
