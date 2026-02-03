// Copyright 2024 the JSR authors. All rights reserved. MIT license.
import type { UsagesCtx } from "@deno/doc/html-types";
import TbChevronRight from "tb-icons/TbChevronRight";

export function Usages({ usages, composed }: UsagesCtx) {
  if (!composed) {
    return (
      <div class="usageContent">
        <h3>Usage</h3>
        <div dangerouslySetInnerHTML={{ __html: usages[0]?.content ?? "" }} />
      </div>
    );
  }

  return (
    <div class="usages">
      {usages.map((usage, index) => (
        <>
          {usage.additional_css && (
            <style scoped>{usage.additional_css}</style>
          )}
          <input
            type="radio"
            name="usage"
            id={usage.name}
            class="hidden"
            checked={index === 0}
          />
        </>
      ))}

      <nav>
        <h3 class="!mb-0">Use with</h3>

        <details id="usageSelector">
          <summary>
            {usages.map((usage) => (
              <div
                id={`${usage.name}_active_dropdown`}
                class="hidden items-center gap-1"
              >
                {usage.icon && (
                  <div
                    class="h-4 *:h-4 *:w-auto flex-none"
                    dangerouslySetInnerHTML={{ __html: usage.icon }}
                  />
                )}
                <div class="leading-none">{usage.name}</div>
              </div>
            ))}

            <div class="rotate-90">
              <TbChevronRight class="size-4" />
            </div>
          </summary>

          <div>
            <div>
              {usages.map((usage) => (
                <label for={usage.name}>
                  {usage.icon && (
                    <div
                      class="h-5 *:h-5 *:w-auto flex-none"
                      dangerouslySetInnerHTML={{ __html: usage.icon }}
                    />
                  )}
                  <div>{usage.name}</div>
                </label>
              ))}
            </div>
          </div>
        </details>
      </nav>

      <div>
        {usages.map((usage) => (
          <div
            id={`${usage.name}_content`}
            class="usageContent"
            dangerouslySetInnerHTML={{ __html: usage.content }}
          />
        ))}
      </div>
    </div>
  );
}
