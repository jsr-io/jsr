// Copyright 2024 the JSR authors. All rights reserved. MIT license.
// deno-lint-ignore-file jsx-curly-braces
import type { IndexSignatureCtx } from "../../../new_html_types.d.ts";
import { Anchor } from "./Anchor.tsx";
import { getDiffColor } from "./mod.ts";

export function IndexSignature(
  { signature }: {
    signature: IndexSignatureCtx;
  },
) {
  const {
    id,
    anchor,
    readonly: isReadonly,
    params,
    ts_type,
    diff_status,
    old_readonly,
    old_ts_type,
  } = signature;

  const typeChanged = old_ts_type !== undefined;
  const readonlyChanged = old_readonly !== undefined;
  const hasChanges = typeChanged || readonlyChanged;

  const diffBg = getDiffColor(diff_status, false);

  return (
    <div class={`anchorable text-sm${diffBg}`} id={id}>
      <Anchor anchor={anchor} />

      {hasChanges && (
        <div class={`diff-removed rounded px-1 py-0.5 mb-0.5`}>
          {(old_readonly ?? isReadonly) && <span>{"readonly "}</span>}
          [<span
            // deno-lint-ignore react-no-danger
            dangerouslySetInnerHTML={{ __html: params }}
          />]
          <span
            // deno-lint-ignore react-no-danger
            dangerouslySetInnerHTML={{ __html: old_ts_type ?? ts_type }}
          />
        </div>
      )}

      <div
        class={hasChanges ? `diff-added rounded px-1 py-0.5` : ""}
      >
        {isReadonly && <span>{"readonly "}</span>}
        [<span
          // deno-lint-ignore react-no-danger
          dangerouslySetInnerHTML={{ __html: params }}
        />]
        <span
          // deno-lint-ignore react-no-danger
          dangerouslySetInnerHTML={{ __html: ts_type }}
        />
      </div>
    </div>
  );
}
