// Copyright 2024 the JSR authors. All rights reserved. MIT license.
export function Cross(props: { class?: string }) { // Size not normalized
  return (
    <svg
      class={`size-6 ${props.class ?? ""}`}
      aria-hidden="true"
      stroke="currentColor"
      fill="none"
      viewBox="0 0 24 24"
    >
      <path
        strokeLinecap="round"
        strokeLinejoin="round"
        strokeWidth="2"
        d="M6 18L18 6M6 6l12 12"
      />
    </svg>
  );
}
