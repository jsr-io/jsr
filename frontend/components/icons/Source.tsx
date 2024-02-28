// Copyright 2024 the JSR authors. All rights reserved. MIT license.
export function Source(props: { class?: string }) {
  return (
    <svg
      class={`w-4 h-4 ${props.class ?? ""}`}
      viewBox="0 0 14 14"
      fill="none"
      xmlns="http://www.w3.org/2000/svg"
    >
      <path
        d="M3.16675 12.6667H11.3334C11.6428 12.6667 11.9396 12.5438 12.1584 12.325C12.3772 12.1062 12.5001 11.8094 12.5001 11.5V4.20834L9.29175 1H4.33342C4.024 1 3.72725 1.12292 3.50846 1.34171C3.28966 1.5605 3.16675 1.85725 3.16675 2.16667V4.5"
        stroke="currentColor"
        stroke-width="1.4"
        stroke-linecap="round"
        stroke-linejoin="round"
      />
      <path
        d="M6.08334 10.3333L7.83334 8.58334L6.08334 6.83334M3.75 6.83334L2 8.58334L3.75 10.3333M9.00001 1V4.5H12.5L9.00001 1Z"
        stroke="currentColor"
        stroke-width="1.4"
        stroke-linecap="round"
        stroke-linejoin="round"
      />
    </svg>
  );
}
