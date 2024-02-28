// Copyright 2024 the JSR authors. All rights reserved. MIT license.
import { RuntimeCompat } from "../utils/api_types.ts";

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
  { runtimeCompat, labeled, hideUnknown }: {
    runtimeCompat: RuntimeCompat;
    labeled?: boolean;
    hideUnknown?: boolean;
  },
) {
  const hasExplicitCompat = Object.values(runtimeCompat).some((v) => v);
  if (!hasExplicitCompat) return null;

  return (
    <div class="min-w-content font-semibold flex items-center gap-2">
      {labeled && <div>Works with</div>}
      <div class="flex items-center gap-1.5">
        {RUNTIME_COMPAT_KEYS.map(
          ([key, name, icon, w, h]) => {
            const value = runtimeCompat[key];
            if (
              value === false || (hideUnknown && value === undefined)
            ) return null;
            return (
              <div
                class="relative h-5"
                style={`aspect-ratio: ${w} / ${h}`}
                title={`${
                  value === undefined
                    ? "It is unknown whether this package works"
                    : "This package works"
                } with ${name}.`}
              >
                <img
                  src={icon}
                  width={w}
                  height={h}
                  class={`h-5 ${
                    value === undefined ? "filter grayscale opacity-40" : ""
                  }`}
                />
                {value === undefined && (
                  <div class="absolute inset-0 h-full w-full text-blue-700 text-center leading-5 drop-shadow-md font-bold text-xl select-none">
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
