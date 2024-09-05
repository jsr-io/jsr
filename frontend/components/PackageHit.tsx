// Copyright 2024 the JSR authors. All rights reserved. MIT license.
import type { OramaPackageHit } from "../util.ts";
import type { Package } from "../utils/api_types.ts";
import { getScoreBgColorClass } from "../utils/score_ring_color.ts";
import type { ListDisplayItem } from "./List.tsx";
import { RuntimeCompatIndicator } from "./RuntimeCompatIndicator.tsx";

export function PackageHit(pkg: OramaPackageHit | Package): ListDisplayItem {
  return {
    href: `/@${pkg.scope}/${pkg.name}`,
    content: (
      <div class="grow-1 w-full flex flex-col md:flex-row gap-2 justify-between">
        <div class="grow-1">
          <div class="flex gap-2">
            <span class="text-jsr-cyan-700 font-semibold">
              {`@${pkg.scope}/${pkg.name}`}
            </span>
            {(pkg as Package).latestVersion && (
              <span class="text-gray-600 max-w-20 truncate font-bold">
                {`@${(pkg as Package).latestVersion}`}
              </span>
            )}
          </div>
          <div class="text-sm text-jsr-gray-600">
            {pkg.description}
          </div>
        </div>

        <div class="flex items-center gap-4">
          <RuntimeCompatIndicator
            runtimeCompat={pkg.runtimeCompat}
            hideUnknown
            compact
          />

          {pkg.score !== null && (
            <div
              class={`rounded-full aspect-square p-0.5 ${
                getScoreBgColorClass(pkg.score)
              }`}
              style={`background-image: conic-gradient(transparent, transparent ${pkg.score}%, #e7e8e8 ${pkg.score}%)`}
              title="Package score"
            >
              <div class="rounded-full aspect-square bg-white text-xs flex items-center justify-center font-semibold min-w-6">
                {pkg.score}
              </div>
            </div>
          )}
        </div>
      </div>
    ),
  };
}
