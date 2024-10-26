// Copyright 2024 the JSR authors. All rights reserved. MIT license.
export function Folder(props: { class?: string }) {
  return (
    <svg
      class={`w-4 h-4 ${props.class ?? ""}`}
      aria-hidden="true"
      viewBox="0 0 14 14"
      fill="none"
    >
      <g clip-path="url(#clip0_889_453)">
        <path
          d="M12.6001 2.80002H7.00012L5.60012 1.40002H1.40012C0.630122 1.40002 0.00712206 2.03002 0.00712206 2.80002L0.00012207 11.2C0.00012207 11.97 0.630122 12.6 1.40012 12.6H12.6001C13.3701 12.6 14.0001 11.97 14.0001 11.2V4.20002C14.0001 3.43002 13.3701 2.80002 12.6001 2.80002ZM12.6001 11.2H1.40012V4.20002H12.6001V11.2Z"
          fill="#6C6E78"
        />
      </g>
      <defs>
        <clipPath id="clip0_889_453">
          <rect width="14" height="14" fill="white" />
        </clipPath>
      </defs>
    </svg>
  );
}
