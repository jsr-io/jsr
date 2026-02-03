// Copyright 2024 the JSR authors. All rights reserved. MIT license.
import TbSourceCode from "tb-icons/TbSourceCode";

export interface SourceButtonProps {
  href: string;
}

export function SourceButton({ href }: SourceButtonProps) {
  return (
    <a class="sourceButton" href={href}>
      <TbSourceCode class="size-4" />
    </a>
  );
}
