// Copyright 2024 the JSR authors. All rights reserved. MIT license.

const height = 7;
const width = 13;

/** number of pixels each blocks spans */
const sizes = {
  small: 2,
  medium: 4,
  large: 6,
} as const;

export function Logo(
  props: { class?: string; size: keyof typeof sizes },
) {
  const ratio = sizes[props.size];

  return (
    <svg
      xmlns="http://www.w3.org/2000/svg"
      viewBox={`0 0 ${width} ${height}`}
      class={props.class}
      aria-hidden="true"
      width={width * ratio}
      height={height * ratio}
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
