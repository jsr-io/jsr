// Copyright 2024 the JSR authors. All rights reserved. MIT license.
import { ComponentChildren } from "preact";

export function Tooltip({ children, tooltip }: {
  children: ComponentChildren;
  tooltip: string;
}) {
  return (
    <div class="group/tooltip">
      {children}
      <div class="w-full flex justify-center">
        <div class="absolute">
          <div class="hidden group-hover/tooltip:flex relative items-center flex-col top-1.5">
            <div class="size-0 inline-block border-solid border-x-4 border-x-transparent border-b-4 border-b-neutral-800" />
            <div class="rounded-md bg-neutral-800 px-4 py-2 text-white text-sm leading-4">
              {tooltip}
            </div>
          </div>
        </div>
      </div>
    </div>
  );
}
