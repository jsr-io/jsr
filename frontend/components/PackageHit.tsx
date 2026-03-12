// Copyright 2024 the JSR authors. All rights reserved. MIT license.
import type { OramaPackageHit } from "../util.ts";
import type { Package, RuntimeCompat } from "../utils/api_types.ts";
import { getScoreBgColorClass } from "../utils/score_ring_color.ts";
import type { ListDisplayItem } from "./List.tsx";
import { RuntimeCompatIndicator } from "./RuntimeCompatIndicator.tsx";
import TbArchive from "tb-icons/TbArchive";

const runtimeCompatExists = (compat: RuntimeCompat) => {
  return compat?.browser || compat?.deno || compat?.node || compat?.workerd ||
    compat?.bun;
};

export function PackageHit(pkg: OramaPackageHit | Package): ListDisplayItem {
  return {
    href: `/@${pkg.scope}/${pkg.name}`,
    content: (
      <div class="grow w-full flex flex-col md:flex-row gap-2 justify-between">
        <div class="grow">
          <div class="flex flex-wrap items-baseline gap-x-2 mb-2 md:mb-0">
            <span class="text-jsr-cyan-700 dark:text-cyan-400 font-semibold">
              {`@${pkg.scope}/${pkg.name}`}
            </span>
            {(pkg as Package).latestVersion && (
              <div class="text-tertiary max-w-20 truncate font-semibold text-sm">
                {`v${(pkg as Package).latestVersion}`}
              </div>
            )}
            {(pkg as Package).isArchived && (
              <div class="text-xs flex items-center gap-1 bg-jsr-yellow-600 text-white px-2 py-0.5 rounded-full">
                <TbArchive class="size-3" />
                Archived
              </div>
            )}
          </div>
          <div class="text-sm text-tertiary">
            {pkg.description}
          </div>
        </div>

        {(runtimeCompatExists(pkg.runtimeCompat) || pkg.score !== null) && (
          <div class="flex items-center gap-4">
            <RuntimeCompatIndicator
              runtimeCompat={pkg.runtimeCompat}
              hideUnknown
              compact
            />

            {pkg.score !== null && (
              <div
                class={`score-circle rounded-full aspect-square p-0.5 ${
                  getScoreBgColorClass(pkg.score)
                }`}
                style={`--pct: ${pkg.score}%`}
                title="Package score"
              >
                <div class="rounded-full aspect-square bg-white dark:bg-jsr-gray-950 text-xs flex items-center justify-center font-semibold min-w-6">
                  {pkg.score}
                </div>
              </div>
            )}
          </div>
        )}
      </div>
    ),
  };
}
