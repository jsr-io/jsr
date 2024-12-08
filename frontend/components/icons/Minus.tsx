// Copyright 2024 the JSR authors. All rights reserved. MIT license.
export function Minus(props: { class?: string }) {
  return (
    <svg
      class={`w-4 h-4 ${props.class ?? ""}`}
      aria-hidden="true"
      viewBox="0 0 14 14"
      fill="none"
    >
      <path
        fill-rule="evenodd"
        clip-rule="evenodd"
        d="M1 7C1 6.58579 1.33579 6.25 1.75 6.25H12.25C12.6642 6.25 13 6.58579 13 7C13 7.41421 12.6642 7.75 12.25 7.75H1.75C1.33579 7.75 1 7.41421 1 7Z"
        fill="currentColor"
      />
    </svg>
  );
}
