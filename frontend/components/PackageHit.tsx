import type { OramaPackageHit } from "../util.ts";
import type { Package } from "../utils/api_types.ts";
import type { ListDisplayItem } from "./List.tsx";
import { RuntimeCompatIndicator } from "./RuntimeCompatIndicator.tsx";

export function PackageHit(pkg: OramaPackageHit | Package): ListDisplayItem {
  return {
    href: `/@${pkg.scope}/${pkg.name}`,
    content: (
      <div class="grow-1 w-full flex flex-col md:flex-row gap-2 justify-between">
        <div class="grow-1">
          <div class="text-cyan-700 font-semibold">
            {`@${pkg.scope}/${pkg.name}`}
          </div>
          <div class="text-sm text-gray-600">
            {pkg.description}
          </div>
        </div>

        <div class="flex items-center gap-4">
          <RuntimeCompatIndicator
            runtimeCompat={pkg.runtimeCompat}
            hideUnknown
          />

          <div class="rounded border-1.5 border-jsr-cyan-950 text-xs px-2 py-1 flex items-center justify-center">
            {pkg.score}%
          </div>
        </div>
      </div>
    ),
  };
}

