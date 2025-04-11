// Copyright 2024 the JSR authors. All rights reserved. MIT license.

import { URLQuerySearch } from "../(_components)/URLQuerySearch.tsx";
import { useRef } from "preact/hooks";

export function AuditURLQuerySearch(
  { query, sudoOnly }: { query: string; sudoOnly: string | null },
) {
  const formRef = useRef<HTMLFormElement>(null);

  return (
    <URLQuerySearch query={query} formRef={formRef}>
      <label class="flex items-center gap-2 text-nowrap">
        <input
          type="checkbox"
          name="sudoOnly"
          value=""
          checked={sudoOnly !== null}
          onChange={(_) => {
            formRef.current?.submit();
          }}
        />
        <span>Sudo Only</span>
      </label>
    </URLQuerySearch>
  );
}
