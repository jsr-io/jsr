// Copyright 2024 the JSR authors. All rights reserved. MIT license.
import type { DocBlockSubtitleClassCtx } from "@deno/doc/html-types";

export function DocBlockSubtitleClass({ value }: DocBlockSubtitleClassCtx) {
  return (
    <>
      {value.implements && value.implements.length > 0 && (
        <div>
          <span class="type"> implements </span>
          {value.implements.map((impl, index) => (
            <>
              <span dangerouslySetInnerHTML={{ __html: impl }} />
              {index < value.implements!.length - 1 && <span>, </span>}
            </>
          ))}
        </div>
      )}
      {value.extends && (
        <div>
          <span class="type"> extends </span>
          {value.extends.href
            ? (
              <a class="link" href={value.extends.href}>
                {value.extends.symbol}
              </a>
            )
            : <span>{value.extends.symbol}</span>}
          {value.extends.type_args && (
            <span dangerouslySetInnerHTML={{ __html: value.extends.type_args }} />
          )}
        </div>
      )}
    </>
  );
}
