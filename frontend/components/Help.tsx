// Copyright 2024 the JSR authors. All rights reserved. MIT license.

import HelpIcon from "tb-icons/TbHelp";

export function Help({ href }: { href: string }) {
  return (
    <a href={href} class="inline-block align-middle opacity-70">
      <HelpIcon class="size-4" />
    </a>
  );
}
