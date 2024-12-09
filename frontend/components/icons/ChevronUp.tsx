// Copyright 2024 the JSR authors. All rights reserved. MIT license.
export function ChevronUp(props: { class?: string }) {
  return (
    <svg
      class={`h-4 w-4 ${props.class ?? ""}`}
      viewBox="0 0 14 14"
      fill="none"
      xmlns="http://www.w3.org/2000/svg"
    >
      <path
        transform="translate(-1 0) rotate(270 7 7)"
        fill-rule="evenodd"
        clip-rule="evenodd"
        d="M3.96967 12.5303C3.67678 12.2374 3.67678 11.7626 3.96967 11.4697L8.43934 7L3.96967 2.53033C3.67678 2.23744 3.67678 1.76256 3.96967 1.46967C4.26256 1.17678 4.73744 1.17678 5.03033 1.46967L10.0303 6.46967C10.3232 6.76256 10.3232 7.23744 10.0303 7.53033L5.03033 12.5303C4.73744 12.8232 4.26256 12.8232 3.96967 12.5303Z"
        fill="currentColor"
      />
    </svg>
  );
}
