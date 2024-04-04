// Copyright 2024 the JSR authors. All rights reserved. MIT license.
export function Check(props: { class?: string }) {
  return (
    <svg
      class={`size-4 ${props.class ?? ""}`}
      viewBox="0 0 24 24"
      strokeWidth={2}
      stroke="currentColor"
      fill="none"
      strokeLinecap="round"
      strokeLinejoin="round"
    >
      <path stroke="none" d="M0 0h24v24H0z" fill="none" />
      <path d="M5 12l5 5l10 -10" />
    </svg>
  );
}
