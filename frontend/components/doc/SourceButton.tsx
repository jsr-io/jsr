// Copyright 2024 the JSR authors. All rights reserved. MIT license.
import TbSourceCode from "tb-icons/TbSourceCode";

export function SourceButton({ href }: { href: string }) {
  return (
    <a
      class="sourceButton hidden group-hover/sourceable:flex flex-row flex-0 gap-2 items-center absolute top-0 right-2"
      href={href}
    >
      <span class="hidden md:block text-xs leading-none flex-none">
        View code
      </span>
      <TbSourceCode class="size-4 flex-none" />
    </a>
  );
}
