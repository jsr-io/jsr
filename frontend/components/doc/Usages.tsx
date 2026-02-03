// Copyright 2024 the JSR authors. All rights reserved. MIT license.
import type { UsagesCtx } from "@deno/doc/html-types";
import TbChevronRight from "tb-icons/TbChevronRight";

export function Usages({ usages: { usages } }: { usages: UsagesCtx }) {
  return (
    <div class="usages">
      {usages.map((usage, index) => (
        <>
          {usage.additional_css && (
            <style scoped
              // css
              // deno-lint-ignore react-no-danger
              dangerouslySetInnerHTML={{ __html: usage.additional_css }} />
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
                  <div class="h-4 *:h-4 *:w-auto flex-none">
                    <img src={usage.icon} alt={`${usage.name} logo`} draggable={false} />
                  </div>
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
                    <div class="h-5 *:h-5 *:w-auto flex-none">
                      <img src={usage.icon} alt={`${usage.name} logo`} draggable={false} />
                    </div>
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
            // usage markdown content
            // deno-lint-ignore react-no-danger
            dangerouslySetInnerHTML={{ __html: usage.content }}
          />
        ))}
      </div>
    </div>
  );
}
