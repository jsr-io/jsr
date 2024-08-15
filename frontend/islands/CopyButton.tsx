// Copyright 2024 the JSR authors. All rights reserved. MIT license.
import { useSignal } from "@preact/signals";
import { useCallback, useRef } from "preact/hooks";
import { Check } from "../components/icons/Check.tsx";
import { Copy } from "../components/icons/Copy.tsx";

interface CopyButtonProps {
  title: string;
  text: string;
}

export function CopyButton(props: CopyButtonProps) {
  const timer = useRef<number | null>(null);
  const copied = useSignal(false);

  const { text } = props;

  const copy = useCallback(() => {
    navigator.clipboard.writeText(text).then(() => {
      copied.value = true;
      if (timer.current) clearTimeout(timer.current);
      timer.current = setTimeout(() => {
        copied.value = false;
      }, 1000);
    });
  }, [text]);

  return (
    <button
      onClick={copy}
      title={props.title}
      class={(copied.value ? "text-green-700" : "text-jsr-gray-700") +
        " hover:bg-jsr-gray-100/30 p-1.5 -mx-1.5 -my-1 rounded-full"}
    >
      {copied.value ? <Check /> : <Copy />}
    </button>
  );
}
