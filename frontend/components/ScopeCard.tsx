// Copyright 2024 the JSR authors. All rights reserved. MIT license.
import { Scope } from "../utils/api_types.ts";
import { Card } from "./Card.tsx";

interface ScopeCardProps {
  scope: Scope;
}

export function ScopeCard({ scope }: ScopeCardProps) {
  return (
    <Card href={`/@${scope.scope}`}>
      <p class="font-semibold text-lg">@{scope.scope}</p>
      {scope.description && (
        <p class="text-sm text-secondary mt-1 line-clamp-2">
          {scope.description}
        </p>
      )}
    </Card>
  );
}
