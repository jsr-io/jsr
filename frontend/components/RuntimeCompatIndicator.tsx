// Copyright 2024 the JSR authors. All rights reserved. MIT license.
import type { RuntimeCompat } from "../utils/api_types.ts";

export const RUNTIME_COMPAT_KEYS: [
  key: keyof RuntimeCompat,
  name: string,
  icon: string,
  width: number,
  height: number,
][] = [
  ["browser", "Browsers", "/logos/browsers.svg", 1200, 500],
  ["deno", "Deno", "/logos/deno.svg", 512, 512],
  ["node", "Node.js", "/logos/node.svg", 256, 292],
  ["workerd", "Cloudflare Workers", "/logos/cloudflare-workers.svg", 416, 375],
  ["bun", "Bun", "/logos/bun.svg", 435, 435],
];

export function RuntimeCompatIndicator(
  { runtimeCompat, hideUnknown, compact }: {
    runtimeCompat: RuntimeCompat;
    hideUnknown?: boolean;
    compact?: boolean;
  },
) {
  const hasExplicitCompat = Object.values(runtimeCompat).some((v) => v);
  if (!hasExplicitCompat) return null;

  const worksWithArray: string[] = [];
  const unknownWithArray: string[] = [];

  for (const [key, name] of RUNTIME_COMPAT_KEYS.toReversed()) {
    const status = runtimeCompat[key];

    if (status) {
      worksWithArray.push(name);
      continue;
    }
    if (status === undefined) {
      unknownWithArray.push(name);
      continue;
    }
  }

  return (
    <div class="min-w-content font-semibold select-none">
      <div
        class={`flex items-center ${
          compact ? "*:-mx-1" : "*:mx-0.5"
        } flex-row-reverse`}
      >
        {worksWithArray.length > 0 && (
          <span className="sr-only">
            This package works with {worksWithArray.join(", ")}
          </span>
        )}
        {unknownWithArray.length > 0 && (
          <span className="sr-only">
            It is unknown whether this package works with{" "}
            {unknownWithArray.join(", ")}
          </span>
        )}
        {RUNTIME_COMPAT_KEYS.toReversed().map(
          ([key, _name, icon, w, h]) => {
            const value = runtimeCompat[key];
            if (
              value === false || (hideUnknown && value === undefined)
            ) return null;
            return (
              <div
                class="relative h-4 md:h-5"
                style={`aspect-ratio: ${w} / ${h}`}
              >
                <img
                  src={icon}
                  width={w}
                  height={h}
                  alt=""
                  class={`h-4 md:h-5 select-none ${
                    value === undefined ? "filter grayscale opacity-40" : ""
                  }`}
                />
                {value === undefined && (
                  <div
                    aria-hidden="true"
                    class="absolute inset-0 h-full w-full text-jsr-cyan-600 text-center leading-4 md:leading-5 drop-shadow-md font-bold text-md md:text-xl select-none"
                  >
                    ?
                  </div>
                )}
              </div>
            );
          },
        )}
      </div>
    </div>
  );
}
