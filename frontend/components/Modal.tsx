// Copyright 2024 the JSR authors. All rights reserved. MIT license.
import { useState } from "preact/hooks";
import { Cross } from "./Icons.tsx";
import { ComponentChildren } from "preact";

export default function Modal(
  { summary, children }: {
    summary: ComponentChildren;
    children: ComponentChildren;
  },
) {
  const [open, setOpen] = useState(false);

  return (
    <details
      open={open}
      class="cursor-pointer"
      onToggle={(e) => {
        if (e.currentTarget.open) {
          setOpen(true);
        }
      }}
    >
      {summary}
      <div
        class="fixed inset-0 bg-[#25252525] z-50 flex items-center justify-center cursor-default max-h-screen p-16"
        onClick={(e) => {
          e.stopPropagation();
          setOpen(false);
        }}
      >
        <div
          class="p-8 relative border border-jsr-cyan-300 rounded bg-white max-w-screen-sm"
          onClick={(e) => e.stopPropagation()}
        >
          <button
            class="absolute top-8 right-8 hover:text-jsr-gray-400"
            onClick={() => setOpen(false)}
          >
            <Cross />
          </button>
          {children}
        </div>
      </div>
    </details>
  );
}
