// Copyright 2024 the JSR authors. All rights reserved. MIT license.
import TbSourceCode from "tb-icons/TbSourceCode";

export function SourceButton({ href }: { href: string }) {
  return (
    <a class="sourceButton" href={href}>
      <TbSourceCode class="size-4" />
    </a>
  );
}
