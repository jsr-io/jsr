// Copyright 2024 the JSR authors. All rights reserved. MIT license.
import { useSignal } from "@preact/signals";
import type { RuntimeCompat } from "../utils/api_types.ts";
import { RUNTIME_COMPAT_KEYS } from "../components/RuntimeCompatIndicator.tsx";

interface SearchFiltersProps {
  runtimes: (keyof RuntimeCompat)[];
  minScore: number | null;
  search: string;
}

export function SearchFilters(
  { runtimes: initialRuntimes, minScore: initialMinScore, search }:
    SearchFiltersProps,
) {
  const expanded = useSignal(
    initialRuntimes.length > 0 || initialMinScore !== null,
  );
  const runtimes = useSignal<(keyof RuntimeCompat)[]>(initialRuntimes);
  const minScore = useSignal<number>(initialMinScore ?? 0);

  function applyFilters() {
    const params = new URLSearchParams();
    if (search) params.set("search", search);
    for (const r of runtimes.value) {
      params.append("runtime", r);
    }
    if (minScore.value > 0) {
      params.set("minScore", String(minScore.value));
    }
    const qs = params.toString();
    location.href = `/packages${qs ? `?${qs}` : ""}`;
  }

  return (
    <div class="mt-4">
      <button
        type="button"
        onClick={() => expanded.value = !expanded.value}
        class="flex items-center gap-1.5 text-sm font-medium text-secondary hover:text-primary transition-colors"
      >
        <svg
          xmlns="http://www.w3.org/2000/svg"
          fill="none"
          viewBox="0 0 24 24"
          stroke-width={2}
          stroke="currentColor"
          class={`w-4 h-4 transition-transform ${
            expanded.value ? "rotate-180" : ""
          }`}
        >
          <path
            strokeLinecap="round"
            strokeLinejoin="round"
            d="M19.5 8.25l-7.5 7.5-7.5-7.5"
          />
        </svg>
        Filters
        {(runtimes.value.length > 0 || minScore.value > 0) && (
          <span class="bg-jsr-cyan-100 dark:bg-jsr-cyan-900 text-jsr-cyan-700 dark:text-jsr-cyan-300 text-xs px-1.5 py-0.5 rounded-full">
            {runtimes.value.length + (minScore.value > 0 ? 1 : 0)}
          </span>
        )}
      </button>

      {expanded.value && (
        <div class="mt-3 p-4 border border-jsr-cyan-100 dark:border-jsr-cyan-900 rounded-lg bg-white dark:bg-jsr-gray-950 space-y-4">
          <div>
            <span class="block text-sm font-medium text-primary mb-2">
              Runtime compatibility
            </span>
            <div class="flex flex-wrap gap-2">
              {RUNTIME_COMPAT_KEYS.map(
                ([key, name, icon, w, h]) => {
                  const checked = runtimes.value.includes(key);
                  return (
                    <button
                      key={key}
                      type="button"
                      onClick={() => {
                        if (checked) {
                          runtimes.value = runtimes.value.filter((r) =>
                            r !== key
                          );
                        } else {
                          runtimes.value = [...runtimes.value, key];
                        }
                        applyFilters();
                      }}
                      class={`inline-flex items-center gap-1.5 px-3 py-1.5 rounded-full border text-sm cursor-pointer select-none transition-colors ${
                        checked
                          ? "border-jsr-cyan-600 bg-jsr-cyan-50 dark:bg-jsr-cyan-950 text-jsr-cyan-700 dark:text-jsr-cyan-300"
                          : "border-jsr-gray-200 dark:border-jsr-gray-700 text-secondary hover:border-jsr-cyan-400 dark:hover:border-jsr-cyan-600"
                      }`}
                    >
                      <img
                        src={icon}
                        width={w}
                        height={h}
                        alt=""
                        class="h-4"
                        style={`aspect-ratio: ${w} / ${h}`}
                      />
                      {name}
                    </button>
                  );
                },
              )}
            </div>
          </div>

          <div>
            <label
              htmlFor="min-score"
              class="block text-sm font-medium text-primary mb-2"
            >
              Minimum score
            </label>
            <div class="flex items-center gap-3">
              <input
                type="range"
                id="min-score"
                min="0"
                max="100"
                step="10"
                value={minScore.value}
                class="w-48 accent-jsr-cyan-600"
                onInput={(e) => {
                  const val = parseInt(
                    (e.currentTarget as HTMLInputElement).value,
                    10,
                  );
                  minScore.value = val;
                }}
                onChange={() => {
                  applyFilters();
                }}
              />
              <span class="text-sm font-medium text-secondary min-w-[2.5rem]">
                {minScore.value === 0 ? "Any" : minScore.value}
              </span>
            </div>
          </div>

          {(runtimes.value.length > 0 || minScore.value > 0) && (
            <div>
              <a
                href={`/packages${search ? `?search=${encodeURIComponent(search)}` : ""}`}
                class="text-sm text-secondary hover:text-primary underline"
              >
                Clear filters
              </a>
            </div>
          )}
        </div>
      )}
    </div>
  );
}
