// Copyright 2024 the JSR authors. All rights reserved. MIT license.
// deno-lint-ignore-file jsx-curly-braces
import type { IndexSignatureCtx } from "@deno/doc/html-types";
import { Anchor } from "./Anchor.tsx";
import { getDiffColor } from "./mod.ts";

export function IndexSignature(
  { signature }: {
    signature: IndexSignatureCtx;
  },
) {
  const {
    anchor,
    diff_status,
    old_readonly,
    readonly,
    old_params,
    params,
    old_ts_type,
    ts_type,
  } = signature;

  let readonlyClass;

  if (readonly == old_readonly) {
    readonlyClass = "";
  } else if (readonly) {
    readonlyClass = "diff-added ml-3";
  } else if (old_readonly) {
    readonlyClass = "diff-removed ml-3";
  }

  return (
    <div
      class={`anchorable text-sm ${getDiffColor(diff_status, false)}`}
      id={anchor.id}
    >
      <Anchor anchor={anchor} />

      <div>
        {(old_readonly ?? readonly) && (
          <span class={readonlyClass}>{"readonly "}</span>
        )}
        [<span>
          {old_params && (
            <span
              class="diff-removed"
              // deno-lint-ignore react-no-danger
              dangerouslySetInnerHTML={{ __html: old_params }}
            />
          )}
          <span
            class={old_params ? "diff-added" : ""}
            // deno-lint-ignore react-no-danger
            dangerouslySetInnerHTML={{ __html: params }}
          />
        </span>]
        <span>
          {old_ts_type && (
            <span
              class="diff-removed"
              // deno-lint-ignore react-no-danger
              dangerouslySetInnerHTML={{ __html: old_ts_type }}
            />
          )}
          <span
            class={old_ts_type ? "diff-added" : ""}
            // deno-lint-ignore react-no-danger
            dangerouslySetInnerHTML={{ __html: ts_type }}
          />
        </span>
      </div>
    </div>
  );
}
