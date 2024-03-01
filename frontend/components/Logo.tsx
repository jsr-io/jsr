// Copyright 2024 the JSR authors. All rights reserved. MIT license.
export function Logo(props: { class?: string }) {
  return (
    <svg
      xmlns="http://www.w3.org/2000/svg"
      viewBox="-1.5 -0.5 16 8"
      class={props.class}
      aria-hidden="true"
    >
      <path
        d="M0,2h2v-2h7v1h4v4h-2v2h-7v-1h-4"
        fill="#083344"
      />
      <path
        d="M1.5,3.5v1h2v-3m4,0h-2v2h2v2h-2m4,0v-3h2v1"
        fill="none"
        stroke="#f7df1e"
        stroke-linecap="square"
      />
    </svg>
  );
}
