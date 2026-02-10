// Copyright 2024 the JSR authors. All rights reserved. MIT license.
import TbSourceCode from "tb-icons/TbSourceCode";

export function SourceButton({ href }: { href: string }) {
  return (
    <a
      class="sourceButton group-hover/sourceable:flex flex-row gap-2 items-center relative mr-2 hidden before:content-['View_code'] before:hidden before:md:block before:text-xs before:leading-none"
      href={href}
    >
      <TbSourceCode class="size-4" />
    </a>
  );
}
