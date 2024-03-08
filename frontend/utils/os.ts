// Copyright 2024 the JSR authors. All rights reserved. MIT license.

import { IS_BROWSER } from "$fresh/runtime.ts";

export function useMacLike(): boolean | undefined {
  if (!IS_BROWSER) return undefined;
  return !!window.navigator.platform.match(/(Mac|iPhone|iPod|iPad)/i);
}
