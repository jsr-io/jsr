// Copyright 2024 the JSR authors. All rights reserved. MIT license.

const height = 7;

/** number of pixels each blocks spans */
const sizes = {
  small: 2,
  medium: 4,
  large: 8,
} as const;

export function Logo(
  props: { class?: string; size: keyof typeof sizes },
) {
  return (
    <>
      <span class="sr-only">JSR</span>
      <svg
        viewBox={`0 0 13 ${height}`}
        class={props.class}
        aria-hidden="true"
        height={height * sizes[props.size]}
      >
        <path
          d="M0,2h2v-2h7v1h4v4h-2v2h-7v-1h-4"
          fill="#083344"
        />
        <g fill="#f7df1e">
          <path d="M1,3h1v1h1v-3h1v4h-3" />
          <path d="M5,1h3v1h-2v1h2v3h-3v-1h2v-1h-2" />
          <path d="M9,2h3v2h-1v-1h-1v3h-1" />
        </g>
      </svg>
    </>
  );
}
