// Copyright 2024 the JSR authors. All rights reserved. MIT license.
import { useRef } from "preact/hooks";
import type { ComponentChildren } from "preact";
import { useSignal } from "@preact/signals";
import { TbCheck, TbCopy } from "tb-icons";

interface CopyButtonProps {
  value: string;
  label: string;
  children?: ComponentChildren;
}

export function CopyButton(props: CopyButtonProps) {
  const timer = useRef<number | null>(null);
  const checked = useSignal(false);

  return (
    <button
      type="button"
      class="rounded-full bg-neutral-100 dark:bg-jsr-gray-700 font-mono hover:bg-neutral-200 dark:hover:bg-jsr-gray-600 cursor-pointer flex items-center justify-center gap-1 p-1"
      aria-label={props.label}
      onClick={() => {
        navigator.clipboard.writeText(props.value);
        checked.value = true;
        if (typeof timer.current === "number") clearTimeout(timer.current);
        timer.current = setTimeout(() => {
          checked.value = false;
        }, 1000);
      }}
    >
      {checked.value
        ? <TbCheck class="size-4 text-green-500" />
        : <TbCopy class="size-4 text-neutral-600 dark:text-neutral-300" />}
      {props.children}
    </button>
  );
}
