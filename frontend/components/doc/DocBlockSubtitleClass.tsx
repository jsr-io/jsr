// Copyright 2024 the JSR authors. All rights reserved. MIT license.
// deno-lint-ignore-file jsx-curly-braces
import type { DocBlockSubtitleClassCtx } from "@deno/doc/html-types";

export function DocBlockSubtitleClass(
  { subtitle: { value } }: { subtitle: DocBlockSubtitleClassCtx },
) {
  return (
    <>
      {value.is_abstract_change && (
        <>
          {value.is_abstract_change.old && (
            <div class={`diff-removed rounded px-1 py-0.5 inline-block`}>
              <span class="text-stone-400 italic dark:text-stone-500">
                {"abstract"}
              </span>
            </div>
          )}
          {value.is_abstract_change.new && (
            <div
              class={`diff-added rounded px-1 py-0.5 inline-block`}
            >
              <span class="text-stone-400 italic dark:text-stone-500">
                {"abstract"}
              </span>
            </div>
          )}
        </>
      )}

      {value.implements_removed && value.implements_removed.length > 0 && (
        <div class={`diff-removed rounded px-1 py-0.5`}>
          <span class="text-stone-400 italic dark:text-stone-500">
            {" implements "}
          </span>
          {value.implements_removed.map((impl, index) => (
            <>
              <span
                // includes type defs which are generated with spans
                // deno-lint-ignore react-no-danger
                dangerouslySetInnerHTML={{ __html: impl }}
              />
              {index < value.implements_removed!.length - 1 && (
                <span>{", "}</span>
              )}
            </>
          ))}
        </div>
      )}

      {value.implements && value.implements.length > 0 && (
        <div>
          <span class="text-stone-400 italic dark:text-stone-500">
            {" implements "}
          </span>
          {value.implements.map((impl, index) => (
            <>
              <span
                class={value.implements_added?.includes(impl)
                  ? `diff-added rounded px-0.5`
                  : ""}
                // includes type defs which are generated with spans
                // deno-lint-ignore react-no-danger
                dangerouslySetInnerHTML={{ __html: impl }}
              />
              {index < value.implements!.length - 1 && <span>{", "}</span>}
            </>
          ))}
        </div>
      )}

      {value.extends_change
        ? (
          <>
            {value.extends_change.old !== null && (
              <div class={`diff-removed rounded px-1 py-0.5`}>
                <span class="text-stone-400 italic dark:text-stone-500">
                  {" extends "}
                </span>
                <span
                  // includes type defs which are generated with spans
                  // deno-lint-ignore react-no-danger
                  dangerouslySetInnerHTML={{ __html: value.extends_change.old }}
                />
              </div>
            )}
            {value.extends_change.new !== null && (
              <div class={`diff-added rounded px-1 py-0.5`}>
                <span class="text-stone-400 italic dark:text-stone-500">
                  {" extends "}
                </span>
                <span
                  // includes type defs which are generated with spans
                  // deno-lint-ignore react-no-danger
                  dangerouslySetInnerHTML={{ __html: value.extends_change.new }}
                />
              </div>
            )}
          </>
        )
        : value.extends && (
          <div>
            <span class="text-stone-400 italic dark:text-stone-500">
              {" extends "}
            </span>
            {value.extends.href
              ? (
                <a class="link" href={value.extends.href}>
                  {value.extends.symbol}
                </a>
              )
              : <span>{value.extends.symbol}</span>}
            {value.extends.type_args && (
              <span
                // includes type defs which are generated with spans
                // deno-lint-ignore react-no-danger
                dangerouslySetInnerHTML={{ __html: value.extends.type_args }}
              />
            )}
          </div>
        )}

      {value.super_type_params_removed &&
        value.super_type_params_removed.length > 0 && (
        <div class={`diff-removed rounded px-1 py-0.5`}>
          {value.super_type_params_removed.map((param, index) => (
            <>
              <span
                // includes type defs which are generated with spans
                // deno-lint-ignore react-no-danger
                dangerouslySetInnerHTML={{ __html: param }}
              />
              {index < value.super_type_params_removed!.length - 1 && (
                <span>{", "}</span>
              )}
            </>
          ))}
        </div>
      )}

      {value.super_type_params_added &&
        value.super_type_params_added.length > 0 && (
        <div class={`diff-added rounded px-1 py-0.5`}>
          {value.super_type_params_added.map((param, index) => (
            <>
              <span
                // includes type defs which are generated with spans
                // deno-lint-ignore react-no-danger
                dangerouslySetInnerHTML={{ __html: param }}
              />
              {index < value.super_type_params_added!.length - 1 && (
                <span>{", "}</span>
              )}
            </>
          ))}
        </div>
      )}
    </>
  );
}
