// Copyright 2024 the JSR authors. All rights reserved. MIT license.

import { URLQuerySearch } from "../(_components)/URLQuerySearch.tsx";
import { useRef } from "preact/hooks";

export function AuditURLQuerySearch(
  { query, sudoOnly }: { query: string; sudoOnly: string | null },
) {
  const formRef = useRef<HTMLFormElement>(null);

  return (
    <URLQuerySearch query={query} formRef={formRef}>
      <label class="flex items-center gap-2 text-nowrap select-none dark:text-gray-200">
        <input
          type="checkbox"
          name="sudoOnly"
          value=""
          checked={sudoOnly !== null}
          onChange={(_) => {
            formRef.current?.submit();
          }}
          class="dark:bg-jsr-gray-700 dark:border-gray-500"
        />
        <span>Sudo Only</span>
      </label>
    </URLQuerySearch>
  );
}
