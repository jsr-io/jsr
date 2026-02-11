// Copyright 2024 the JSR authors. All rights reserved. MIT license.
import { asset } from "fresh/runtime";
import type { UsagesCtx } from "@deno/doc/html-types";
import TbChevronRight from "tb-icons/TbChevronRight";
import { useComputed, useSignal, useSignalEffect } from "@preact/signals";
import { useEffect } from "preact/hooks";

export default function DocUsages(
  { usages: { usages } }: { usages: UsagesCtx },
) {
  const activeUsageIdx = useSignal(0);
  const activeUsage = useComputed(() => usages[activeUsageIdx.value]);

  useEffect(() => {
    const preferredUsage = localStorage.getItem("preferredUsage");

    if (preferredUsage) {
      activeUsageIdx.value = usages.findIndex((usage) =>
        usage.name === preferredUsage
      );
    }
  }, []);

  useSignalEffect(() => {
    localStorage.setItem("preferredUsage", activeUsage.value.name);
  });

  return (
    <div class="usages">
      <nav class="flex items-center flex-row gap-2 mb-3 font-semibold">
        <h3 class="font-bold text-lg">Use with</h3>

        <details id="usageSelector">
          <summary class="flex gap-1 select-none cursor-pointer rounded border py-2 px-3 border-jsr-cyan-300/50 bg-jsr-cyan-50 dark:border-jsr-cyan-800 dark:bg-jsr-cyan-950">
            <div class="flex items-center gap-1">
              {activeUsage.value.icon && (
                <div class="h-4 *:h-4 *:w-auto flex-none">
                  <img
                    src={asset(activeUsage.value.icon)}
                    alt={`${activeUsage.value.name} logo`}
                    draggable={false}
                  />
                </div>
              )}
              <div class="leading-none">{activeUsage.value.name}</div>
            </div>

            <div class="rotate-90">
              <TbChevronRight class="size-4" />
            </div>
          </summary>

          <div class="md:relative">
            <div class="absolute max-md:inset-x-0 mt-1.5 p-2 block z-30 md:w-48 bg-white md:rounded border max-md:border-x-0 border-jsr-cyan-200 dark:border-jsr-cyan-800 dark:bg-jsr-gray-950">
              {usages.map((usage, i) => (
                <label class="flex items-center gap-2 cursor-pointer select-none px-2 py-1 leading-normal rounded-sm hover:bg-gray-50 dark:hover:bg-gray-800">
                  <input
                    type="radio"
                    id={usage.name}
                    class="hidden"
                    onClick={() => activeUsageIdx.value = i}
                  />

                  {usage.icon && (
                    <div class="h-5 *:h-5 *:w-auto flex-none">
                      <img
                        src={asset(usage.icon)}
                        alt={`${usage.name} logo`}
                        draggable={false}
                      />
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
        <div
          id={`${activeUsage.value.name}_content`}
          class="usageContent"
          // usage markdown content
          // deno-lint-ignore react-no-danger
          dangerouslySetInnerHTML={{ __html: activeUsage.value.content }}
        />
      </div>
    </div>
  );
}
