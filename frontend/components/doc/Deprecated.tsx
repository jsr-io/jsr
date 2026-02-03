// Copyright 2024 the JSR authors. All rights reserved. MIT license.

export interface DeprecatedProps {
  /** HTML string or null/undefined if not deprecated */
  message?: string | null;
}

export function Deprecated({ message }: DeprecatedProps) {
  if (message === undefined || message === null) {
    return null;
  }

  return (
    <div class="deprecated">
      <div>
        <span>Deprecated</span>
      </div>
      {message !== "" && (
        <div dangerouslySetInnerHTML={{ __html: message }} />
      )}
    </div>
  );
}
