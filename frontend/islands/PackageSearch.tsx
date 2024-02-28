// Copyright 2024 the JSR authors. All rights reserved. MIT license.
import { computed, Signal, useSignal } from "@preact/signals";
import { useEffect, useMemo, useRef } from "preact/hooks";
import { JSX } from "preact/jsx-runtime";
import { OramaClient } from "@oramacloud/client";
import { IS_BROWSER } from "$fresh/runtime.ts";
import { OramaPackageHit } from "../util.ts";
import { api, path } from "../utils/api.ts";
import { List, Package } from "../utils/api_types.ts";

interface PackageSearchProps {
  query?: string;
  indexId?: string;
  apiKey?: string;
  jumbo?: boolean;
}

// 450ms is a suggestion from Michele at Orama.
const TYPING_DEBOUNCE = 450;

export function PackageSearch(
  { query, indexId, apiKey, jumbo }: PackageSearchProps,
) {
  const suggestions = useSignal<(OramaPackageHit[] | Package[])>([]);
  const pending = useSignal(false);
  const debounceRef = useRef(-1);
  const abort = useRef<AbortController | null>(null);
  const selectionIdx = useSignal(-1);
  const ref = useRef<HTMLDivElement>(null);
  const showSuggestions = useSignal(true);
  const btnSubmit = useSignal(false);
  const sizeClasses = jumbo ? "py-3 px-4 text-lg" : "py-1 px-2 text-base";

  const orama = useMemo(() => {
    if (IS_BROWSER && indexId) {
      return new OramaClient({
        endpoint: `https://cloud.orama.run/v1/indexes/${indexId}`,
        api_key: apiKey!,
      });
    }
  }, [indexId, apiKey]);

  useEffect(() => {
    const outsideClick = (e: Event) => {
      if (!ref.current) return;
      showSuggestions.value = ref.current.contains(e.target as Element);
    };

    document.addEventListener("click", outsideClick);
    return () => document.removeEventListener("click", outsideClick);
  }, []);

  const onInput = (ev: JSX.TargetedEvent<HTMLInputElement>) => {
    const value = ev.currentTarget!.value as string;
    if (value.length > 1) {
      showSuggestions.value = true;
      pending.value = true;
      selectionIdx.value = -1;
      abort.current?.abort();
      abort.current = new AbortController();
      clearTimeout(debounceRef.current);
      debounceRef.current = setTimeout(async () => {
        selectionIdx.value = -1;
        try {
          if (orama) {
            const res = await orama.search({
              term: value,
              limit: 5,
              mode: "fulltext",
            }, {
              // @ts-ignore same named AbortController, but different?
              abortController: abort.current,
            });
            suggestions.value = res?.hits.map((hit) => hit.document) ?? [];
          } else {
            const res = await api.get<List<Package>>(path`/packages`, {
              query: value,
              limit: 5,
            });
            pending.value = false;
            if (res.ok) {
              suggestions.value = res.data.items;
            } else {
              throw res;
            }
          }
        } catch (_e) {
          suggestions.value = [];
        }

        pending.value = false;
      }, TYPING_DEBOUNCE);
    } else {
      abort.current?.abort();
      abort.current = new AbortController();
      clearTimeout(debounceRef.current);
      pending.value = false;
      suggestions.value = [];
    }
  };

  function onKeyUp(e: KeyboardEvent) {
    if (pending.value) return;

    if (e.key === "ArrowDown") {
      selectionIdx.value = Math.min(
        suggestions.value.length - 1,
        selectionIdx.value + 1,
      );
    } else if (e.key === "ArrowUp") {
      selectionIdx.value = Math.max(0, selectionIdx.value - 1);
    }
  }

  function onSubmit(e: JSX.TargetedEvent<HTMLFormElement>) {
    if (!btnSubmit.value && selectionIdx.value > -1) {
      const item = suggestions.value[selectionIdx.value];
      if (item !== undefined) {
        e.preventDefault();
        location.href =
          new URL(`/@${item.scope}/${item.name}`, location.origin).href;
      }
    }
  }

  return (
    <div ref={ref}>
      <form
        action="/packages"
        method="GET"
        class="flex w-full"
        onSubmit={onSubmit}
      >
        <input
          type="text"
          name="search"
          aria-label="Search for packages"
          class={`block w-full search-input bg-white/90 input rounded-r-none ${sizeClasses}`}
          placeholder="Search for packages"
          value={query}
          onInput={onInput}
          onKeyUp={onKeyUp}
          onFocus={() => showSuggestions.value = true}
          autoComplete="off"
          aria-expanded="false"
        />

        <button
          type="submit"
          class="button bg-cyan-950 text-white px-4 rounded-l-none hover:bg-cyan-800 focus-visible:bg-cyan-800 outline-cyan-600 transition-colors duration-150"
          onMouseDown={() => {
            btnSubmit.value = true;
          }}
          onTouchStart={() => {
            btnSubmit.value = true;
          }}
        >
          <svg
            xmlns="http://www.w3.org/2000/svg"
            fill="none"
            viewBox="0 0 24 24"
            stroke-width={2.5}
            aria-label="search"
            stroke="currentColor"
            class="w-4 h-4"
          >
            <path
              strokeLinecap="round"
              strokeLinejoin="round"
              d="M21 21l-5.197-5.197m0 0A7.5 7.5 0 105.196 5.196a7.5 7.5 0 0010.607 10.607z"
            />
          </svg>
        </button>
      </form>
      <div role="listbox" tabindex={query?.length ? 0 : -1} class="relative">
        <SuggestionList
          showSuggestions={showSuggestions}
          suggestions={suggestions}
          pending={pending}
          selectionIdx={selectionIdx}
        />
      </div>
    </div>
  );
}

function SuggestionList(
  { suggestions, pending, selectionIdx, showSuggestions }: {
    suggestions: Signal<OramaPackageHit[] | Package[]>;
    showSuggestions: Signal<boolean>;
    pending: Signal<boolean>;
    selectionIdx: Signal<number>;
  },
) {
  if (
    !showSuggestions.value || !pending.value && suggestions.value.length == 0
  ) return null;

  return (
    <div class="absolute bg-white w-full border sibling:bg-red-500 shadow z-40">
      {pending.value ? <div class="bg-white px-4">...</div> : null}
      {!pending.value && (
        <ul class="divide-y-1">
          {suggestions.value.map((pkg, i) => {
            const selected = computed(() => selectionIdx.value === i);
            return (
              <li
                key={pkg.scope + pkg.name}
                class="p-2 hover:bg-gray-100 cursor-pointer aria-[selected=true]:bg-cyan-100"
                aria-selected={selected}
              >
                <a href={`/@${pkg.scope}/${pkg.name}`} class="bg-red-600">
                  <div class="text-cyan-700 font-semibold">
                    @{pkg.scope}/{pkg.name}
                  </div>
                  <div class="text-sm text-gray-500">
                    {pkg.description || "-"}
                  </div>
                </a>
              </li>
            );
          })}
        </ul>
      )}
    </div>
  );
}
