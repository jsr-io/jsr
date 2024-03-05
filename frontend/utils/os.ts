// Copyright 2024 the JSR authors. All rights reserved. MIT license.

export const isMacLike = window.navigator.platform.match(
    /(Mac|iPhone|iPod|iPad)/i,
  )
  ? true
  : false;
