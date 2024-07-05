// Copyright 2024 the JSR authors. All rights reserved. MIT license.
export function ExternalLink(props: { class?: string; strokeWidth?: string }) {
  return (
    <svg
      fill="none"
      stroke-width={props.strokeWidth ?? "1.5"}
      stroke="currentColor"
      viewBox="0 0 24 24"
      xmlns="http://www.w3.org/2000/svg"
      class={`size-4 ${props.class ?? ""}`}
    >
      <path
        stroke-linecap="round"
        stroke-linejoin="round"
        d="M13.5 6H5.25A2.25 2.25 0 0 0 3 8.25v10.5A2.25 2.25 0 0 0 5.25 21h10.5A2.25 2.25 0 0 0 18 18.75V10.5m-10.5 6L21 3m0 0h-5.25M21 3v5.25"
      />
    </svg>
  );
}
