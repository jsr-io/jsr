// Copyright 2024 the JSR authors. All rights reserved. MIT license.
import { Scope } from "../../../utils/api_types.ts";

export interface ScopeHeaderProps {
  scope: Scope;
}

export function ScopeHeader(props: ScopeHeaderProps) {
  return (
    <>
      <h1 class="text-2xl leading-none font-semibold">
        @{props.scope.scope}
      </h1>
      {props.scope.description && (
        <p class="text-secondary mt-2">
          {props.scope.description}
        </p>
      )}
    </>
  );
}
