// Copyright 2024 the JSR authors. All rights reserved. MIT license.
import { batch, computed, Signal, useSignal } from "@preact/signals";
import { useEffect, useMemo, useRef } from "preact/hooks";
import { JSX } from "preact/jsx-runtime";
import { OramaClient } from "@oramacloud/client";
import { Highlight } from "@orama/highlight";
import { IS_BROWSER } from "$fresh/runtime.ts";
import type { OramaPackageHit, SearchKind } from "../util.ts";
import { api, path } from "../utils/api.ts";
import { List, Package } from "../utils/api_types.ts";
import { PackageHit } from "../components/PackageHit.tsx";
import { useMacLike } from "../utils/os.ts";
import type { ListDisplayItem } from "../components/List.tsx";
import { RUNTIME_COMPAT_KEYS } from "../components/RuntimeCompatIndicator.tsx";

interface GlobalSearchProps {
  query?: string;
  indexId?: string;
  apiKey?: string;
  jumbo?: boolean;
  kind?: SearchKind;
}

// The maximum time between a query and the result for that query being
// displayed, if there is a more recent pending query.
const MAX_STALE_RESULT_MS = 200;

export function GlobalSearch(
  {
    query,
    indexId,
    apiKey,
    jumbo,
    kind = "packages",
  }: GlobalSearchProps,
) {
  const suggestions = useSignal<
    OramaPackageHit[] | Package[] | OramaDocsHit[] | null
  >(null);
  const searchNRef = useRef({
    started: 0,
    displayed: 0,
  });
  const abort = useRef<AbortController | null>(null);
  const selectionIdx = useSignal(-1);
  const ref = useRef<HTMLDivElement>(null);
  const isFocused = useSignal(false);
  const search = useSignal(query ?? "");
  const btnSubmit = useSignal(false);
  const sizeClasses = jumbo ? "py-3 px-4 text-lg" : "py-1 px-2 text-base";

  const showSuggestions = computed(() =>
    isFocused.value && search.value.length > 0
  );
  const macLike = useMacLike();

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
      isFocused.value = ref.current.contains(e.target as Element);
    };

    document.addEventListener("click", outsideClick);
    return () => document.removeEventListener("click", outsideClick);
  }, []);

  useEffect(() => {
    const keyboardHandler = (e: KeyboardEvent) => {
      if (((e.metaKey || e.ctrlKey) && e.key === "k")) {
        e.preventDefault();
        (document.querySelector("#global-search-input") as HTMLInputElement)
          ?.focus();
      }
    };
    globalThis.addEventListener("keydown", keyboardHandler);
    return function cleanup() {
      globalThis.removeEventListener("keydown", keyboardHandler);
    };
  });

  const onInput = (ev: JSX.TargetedEvent<HTMLInputElement>) => {
    const value = ev.currentTarget!.value as string;
    search.value = value;
    if (value.length >= 1) {
      const searchN = ++searchNRef.current.started;
      const oldAborter = abort.current;
      abort.current = new AbortController();
      setTimeout(() => {
        oldAborter?.abort();
        if (searchNRef.current.displayed < searchN) {
          selectionIdx.value = -1;
          suggestions.value = null;
        }
      }, MAX_STALE_RESULT_MS);

      (async () => {
        try {
          if (orama) {
            let query = value;
            let where: undefined | Record<string, boolean | string> = undefined;
            if (kind === "packages") ({ where, query } = processFilter(value));
            const res = await orama.search({
              term: query,
              where,
              limit: 5,
              mode: "fulltext",
            }, { abortController: abort.current! });
            if (
              abort.current?.signal.aborted ||
              searchNRef.current.displayed > searchN
            ) {
              return;
            }
            searchNRef.current.displayed = searchN;
            batch(() => {
              selectionIdx.value = -1;
              suggestions.value = res?.hits.map((hit) => hit.document) ?? [];
            });
          } else if (kind === "packages") {
            const res = await api.get<List<Package>>(path`/packages`, {
              query: value,
              limit: 5,
            });
            if (res.ok) {
              if (
                abort.current?.signal.aborted ||
                searchNRef.current.displayed > searchN
              ) {
                return;
              }
              searchNRef.current.displayed = searchN;
              batch(() => {
                selectionIdx.value = -1;
                suggestions.value = res.data.items;
              });
            } else {
              throw res;
            }
          } else {
            suggestions.value = [];
          }
        } catch (_e) {
          if (abort.current?.signal.aborted) return;
          suggestions.value = null;
        }
      })();
    } else {
      abort.current?.abort();
      abort.current = new AbortController();
      suggestions.value = null;
    }
  };

  function onKeyUp(e: KeyboardEvent) {
    if (suggestions.value === null) return;
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
    if (
      !btnSubmit.value && selectionIdx.value > -1 && suggestions.value !== null
    ) {
      const item = suggestions.value[selectionIdx.value];
      if (item !== undefined) {
        e.preventDefault();

        if (kind === "packages") {
          location.href = new URL(
            `/@${(item as (OramaPackageHit | Package)).scope}/${
              (item as (OramaPackageHit | Package)).name
            }`,
            location.origin,
          ).href;
        } else {
          location.href = new URL(
            `/docs/${(item as OramaDocsHit).path}${
              (item as OramaDocsHit).slug
                ? `#${(item as OramaDocsHit).slug}`
                : ""
            }`,
            location.origin,
          ).href;
        }
      }
    }

    if (kind === "docs") {
      e.preventDefault();
    }
  }

  const kindPlaceholder = kind === "packages"
    ? "Search for packages"
    : "Search for documentation";
  const placeholder = kindPlaceholder +
    (macLike !== undefined ? ` (${macLike ? "⌘K" : "Ctrl+K"})` : "");
  return (
    <div ref={ref} class="pointer-events-auto">
      <form
        action={kind === "packages" ? "/packages" : ""}
        method="GET"
        class="flex w-full"
        onSubmit={onSubmit}
      >
        <label htmlFor="global-search-input" class="sr-only">
          {kindPlaceholder}
        </label>
        <input
          type="text"
          name="search"
          class={`block w-full search-input bg-white/90 input rounded-r-none ${sizeClasses} relative`}
          placeholder={placeholder}
          value={query}
          onInput={onInput}
          onKeyUp={onKeyUp}
          onFocus={() => isFocused.value = true}
          autoComplete="off"
          aria-expanded={showSuggestions}
          aria-autocomplete="list"
          aria-controls="package-search-results"
          role="combobox"
          id="global-search-input"
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
      <div
        role="listbox"
        id="global-search-results"
        tabindex={query?.length ? 0 : -1}
        class="relative"
        aria-label="Search results"
      >
        <SuggestionList
          showSuggestions={showSuggestions}
          suggestions={suggestions}
          selectionIdx={selectionIdx}
          kind={kind}
          input={search}
        />
      </div>
    </div>
  );
}

function SuggestionList(
  {
    suggestions,
    selectionIdx,
    showSuggestions,
    kind,
    input,
  }: {
    suggestions: Signal<
      (OramaPackageHit[] | Package[]) | OramaDocsHit[] | null
    >;
    showSuggestions: Signal<boolean>;
    selectionIdx: Signal<number>;
    kind: SearchKind;
    input: Signal<string>;
  },
) {
  if (!showSuggestions.value) return null;

  return (
    <div class="absolute bg-white w-full sibling:bg-red-500 border-1.5 border-jsr-cyan-950 rounded-lg z-40 overflow-hidden top-0.5">
      {suggestions.value === null
        ? <div class="bg-white text-gray-500 px-4">...</div>
        : suggestions.value?.length === 0
        ? (
          <div class="bg-white text-gray-500 px-4 py-2">
            No matching results to display
          </div>
        )
        : (
          <ul class="divide-y-1">
            {suggestions.value.map((rawHit, i) => {
              const selected = computed(() => selectionIdx.value === i);
              const hit = kind === "packages"
                ? PackageHit(rawHit as (OramaPackageHit | Package))
                : DocsHit(rawHit as OramaDocsHit, input);

              return (
                <li
                  key={i}
                  class="p-2 hover:bg-gray-100 cursor-pointer aria-[selected=true]:bg-cyan-100"
                  aria-selected={selected}
                >
                  <a href={hit.href} class="bg-red-600">
                    {hit.content}
                  </a>
                </li>
              );
            })}
          </ul>
        )}
      <div class="bg-gray-100 flex items-center justify-end py-1 px-2 gap-1">
        <span class="text-sm text-gray-500">
          powered by <span class="sr-only">Orama</span>
        </span>
        <img class="h-4" src="/logos/orama-dark.svg" alt="" />
      </div>
    </div>
  );
}

export interface OramaDocsHit {
  path: string;
  header: string;
  headerParts: string[];
  slug: string;
  content: string;
}

function DocsHit(hit: OramaDocsHit, input: Signal<string>): ListDisplayItem {
  const highlighter = new Highlight();

  return {
    href: `/docs/${hit.path}${hit.slug ? `#${hit.slug}` : ""}`,
    content: (
      <div class="grow-1 w-full space-y-1">
        {hit.header && (
          <div class="font-semibold space-x-1">
            {hit.headerParts.map((part, i) => (
              <>
                {i !== 0 && <span>{">"}</span>}
                <span
                  class="text-cyan-700"
                  dangerouslySetInnerHTML={{
                    __html: highlighter.highlight(part, input.value)
                      .HTML,
                  }}
                />
              </>
            ))}
          </div>
        )}
        <div
          class="text-sm text-gray-600"
          dangerouslySetInnerHTML={{
            __html: highlighter.highlight(hit.content, input.value)
              .trim(100),
          }}
        />
      </div>
    ),
  };
}

export function processFilter(
  search: string,
): { query: string; where: Record<string, boolean | string> | undefined } {
  const filters: [string, boolean | string][] = [];
  let query = "";
  for (const part of search.split(" ")) {
    // TODO: enable filtering by scope - currently not supported by Orama?
    // if (part.startsWith("scope:")) {
    //   filters.push(["scope", part.slice(6)]);
    // } else
    if (part.startsWith("runtime:")) {
      const runtime = part.slice(8);
      if (RUNTIME_COMPAT_KEYS.find(([k]) => runtime == k)) {
        filters.push([`runtimeCompat.${runtime}`, true]);
      }
    } else {
      query += part + " ";
    }
  }
  const where = Object.fromEntries(filters);
  return {
    query: query.trim(),
    where: filters.length === 0 ? undefined : where,
  };
}
