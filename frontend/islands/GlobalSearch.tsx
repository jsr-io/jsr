// Copyright 2024 the JSR authors. All rights reserved. MIT license.
import { batch, computed, Signal, useSignal } from "@preact/signals";
import { useEffect, useMemo, useRef } from "preact/hooks";
import { JSX } from "preact/jsx-runtime";
import { liteClient } from "algoliasearch/lite";
import { Highlight } from "@orama/highlight";
import { IS_BROWSER } from "fresh/runtime";
import type { AlgoliaPackageHit, SearchKind } from "../util.ts";
import { api, path } from "../utils/api.ts";
import type { List, Package, RuntimeCompat } from "../utils/api_types.ts";
import { PackageHit } from "../components/PackageHit.tsx";
import { useMacLike } from "../utils/os.ts";
import type { ListDisplayItem } from "../components/List.tsx";
import { RUNTIME_COMPAT_KEYS } from "../components/RuntimeCompatIndicator.tsx";

interface GlobalSearchProps {
  query?: string;
  appId?: string;
  apiKey?: string;
  indexName?: string;
  jumbo?: boolean;
  kind?: SearchKind;
}

const searchHints: JSX.Element[] = [
  <p key="scope:">
    Hint: use <code>scope:</code> to search for packages by scope
  </p>,
  <p key="runtime:">
    Hint: use <code>runtime:</code> to search for packages by compatible runtime
  </p>,
];

// The maximum time between a query and the result for that query being
// displayed, if there is a more recent pending query.
const MAX_STALE_RESULT_MS = 200;

export function GlobalSearch(
  {
    query,
    appId,
    apiKey,
    indexName,
    jumbo,
    kind = "packages",
  }: GlobalSearchProps,
) {
  const suggestions = useSignal<
    AlgoliaPackageHit[] | Package[] | AlgoliaDocsHit[] | null
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
  const inputOverlayContentRef = useRef<HTMLDivElement>(null);
  const inputOverlayContent2Ref = useRef<HTMLDivElement>(null);
  const sizeClasses = jumbo ? "py-3 px-4 text-lg" : "py-1 px-2 text-base";

  const showSuggestions = computed(() =>
    isFocused.value && (search.value.length > 0 || kind !== "docs")
  );
  const macLike = useMacLike();

  const algolia = useMemo(() => {
    if (IS_BROWSER && appId && indexName) {
      return liteClient(appId, apiKey!);
    }
  }, [appId, apiKey]);

  const randomHint = useSignal<JSX.Element | null>(null);

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

  // Initialize random hint once on mount
  useEffect(() => {
    randomHint.value =
      searchHints[Math.floor(Math.random() * searchHints.length)];
  }, []);

  const onInput = (ev: JSX.TargetedEvent<HTMLInputElement>) => {
    const value = ev.currentTarget!.value as string;
    search.value = value;
    updateOverlayScroll(ev.currentTarget! as HTMLInputElement);

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
          if (algolia) {
            let query = value;
            let filters: string | undefined = undefined;
            if (kind === "packages") {
              ({ filters, query } = processFilter(value));
            }
            const { results } = await algolia.search({
              requests: [{
                indexName: indexName!,
                query,
                filters,
                hitsPerPage: 5,
              }],
            });
            if (
              abort.current?.signal.aborted ||
              searchNRef.current.displayed > searchN
            ) {
              return;
            }
            searchNRef.current.displayed = searchN;
            batch(() => {
              selectionIdx.value = -1;
              // deno-lint-ignore no-explicit-any
              suggestions.value = (results[0] as any)?.hits ?? [];
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
    if (
      e.key === "ArrowRight" &&
      (e.currentTarget! as HTMLInputElement).selectionStart ===
        search.value.length &&
      tokenizeFilter(search.value).at(-1)?.kind !== "text"
    ) {
      search.value += " ";
      updateOverlayScroll(e.currentTarget! as HTMLInputElement);
      return;
    }

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

  function updateOverlayScroll(element: HTMLElement) {
    if (inputOverlayContentRef.current && inputOverlayContent2Ref.current) {
      inputOverlayContentRef.current.style.transform = `translateX(${-element
        .scrollLeft}px)`;
      inputOverlayContent2Ref.current.style.transform = `translateX(${-element
        .scrollLeft}px)`;
    }
  }

  function onScroll(e: Event) {
    updateOverlayScroll(e.currentTarget! as HTMLInputElement);
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
            `/@${(item as (AlgoliaPackageHit | Package)).scope}/${
              (item as (AlgoliaPackageHit | Package)).name
            }`,
            location.origin,
          ).href;
        } else {
          location.href = new URL(
            `/docs/${(item as AlgoliaDocsHit).path}${
              (item as AlgoliaDocsHit).slug
                ? `#${(item as AlgoliaDocsHit).slug}`
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
        <div class="relative w-full">
          <input
            type="search"
            name="search"
            class={`w-full h-full search-input bg-white/90 dark:bg-jsr-gray-950/90 truncate ${
              kind === "packages"
                ? "text-transparent! selection:text-transparent selection:bg-blue-500/30 dark:selection:bg-blue-400/40"
                : ""
            } caret-black! dark:caret-white! input rounded-r-none ${sizeClasses} relative`}
            placeholder={placeholder}
            value={search.value}
            onInput={onInput}
            onKeyUp={onKeyUp}
            onFocus={() => isFocused.value = true}
            onScroll={onScroll}
            autoComplete="off"
            aria-expanded={showSuggestions}
            aria-autocomplete="list"
            aria-controls="package-search-results"
            role="combobox"
            id="global-search-input"
          />
          {kind === "packages" && (
            <div
              class={`search-input bg-transparent! border-transparent! select-none pointer-events-none inset-0 absolute ${sizeClasses}`}
            >
              <div
                ref={inputOverlayContentRef}
                class={`whitespace-pre`}
              >
                {tokenizeFilter(search.value).map((token, i, arr) => (
                  <span>
                    <span
                      class={token.kind === "text" ? "" : "search-input-tag"}
                    >
                      {token.raw}
                    </span>
                    {((arr.length - 1) !== i) && " "}
                  </span>
                ))}
              </div>
            </div>
          )}
        </div>

        <button
          type="submit"
          class="button bg-jsr-cyan-950 text-white px-4 rounded-l-none hover:bg-jsr-cyan-800 focus-visible:bg-jsr-cyan-800 outline-jsr-cyan-600 transition-colors duration-150"
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
          randomHint={randomHint}
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
    randomHint,
  }: Readonly<{
    suggestions: Signal<
      (AlgoliaPackageHit[] | Package[]) | AlgoliaDocsHit[] | null
    >;
    showSuggestions: Signal<boolean>;
    selectionIdx: Signal<number>;
    kind: SearchKind;
    input: Signal<string>;
    randomHint: Signal<JSX.Element | null>;
  }>,
) {
  if (!showSuggestions.value) return null;

  return (
    <div class="absolute bg-white dark:bg-jsr-gray-950 w-full sibling:bg-red-500 border-1.5 border-jsr-cyan-950 dark:border-jsr-cyan-600 rounded-lg z-40 overflow-hidden top-0.5">
      {suggestions.value === null && kind === "packages"
        ? (
          <div class="bg-white dark:bg-jsr-gray-950 text-tertiary px-4 py-2">
            {randomHint.value || "Loading..."}
          </div>
        )
        : suggestions.value === null || suggestions.value.length === 0
        ? (
          <div class="bg-white dark:bg-jsr-gray-950 text-tertiary px-4 py-2">
            No matching results to display
          </div>
        )
        : (
          <ul class="divide-y-1 dark:divide-jsr-gray-900">
            {suggestions.value.map((rawHit, i) => {
              const selected = computed(() => selectionIdx.value === i);
              const hit = kind === "packages"
                ? PackageHit(rawHit as (AlgoliaPackageHit | Package))
                : DocsHit(rawHit as AlgoliaDocsHit, input);

              return (
                <li
                  key={i}
                  class="p-2 hover:bg-jsr-gray-100 dark:hover:bg-jsr-gray-900 cursor-pointer aria-selected:bg-jsr-cyan-100 dark:aria-selected:bg-jsr-cyan-950"
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
      <div class="bg-jsr-gray-50 dark:bg-jsr-gray-900 flex items-center justify-between py-1 px-2 text-sm">
        <div>
          {kind === "packages" && (
            <a
              class="link"
              href="/docs/faq#can-i-filter-packages-by-compatible-runtime-in-the-search"
              target="_blank"
            >
              Search syntax
            </a>
          )}
        </div>
        <div class="flex items-center gap-1">
          <span class="text-tertiary">powered by</span>
          <a
            href="https://www.algolia.com/?utm_medium=AOS-referral"
            target="_blank"
            aria-label="Algolia"
          >
            <img class="h-4" src="/logos/algolia.svg" alt="Algolia" />
          </a>
        </div>
      </div>
    </div>
  );
}

export interface AlgoliaDocsHit {
  path: string;
  header: string;
  headerParts: string[];
  slug: string;
  content: string;
}

function DocsHit(hit: AlgoliaDocsHit, input: Signal<string>): ListDisplayItem {
  const highlighter = new Highlight();

  return {
    href: `/docs/${hit.path}${hit.slug ? `#${hit.slug}` : ""}`,
    content: (
      <div class="grow w-full space-y-1">
        {hit.header && (
          <div class="font-semibold space-x-1">
            {hit.headerParts.map((part, i) => (
              <>
                {i !== 0 && <span>&gt;</span>}
                <span
                  class="text-jsr-cyan-700"
                  // deno-lint-ignore react-no-danger
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
          class="text-sm text-secondary"
          // deno-lint-ignore react-no-danger
          dangerouslySetInnerHTML={{
            __html: highlighter.highlight(hit.content, input.value)
              .trim(100),
          }}
        />
      </div>
    ),
  };
}

interface TextToken {
  kind: "text";
  value: string;
  raw: string;
}
interface ScopeToken {
  kind: "scope";
  value: string;
  raw: string;
}
interface RuntimeToken {
  kind: `runtimeCompat.${keyof RuntimeCompat}`;
  value: true;
  raw: string;
}

type Token = TextToken | ScopeToken | RuntimeToken;

function tokenizeFilter(search: string): Token[] {
  const tokens: Token[] = [];

  for (const part of search.split(" ")) {
    if (part.startsWith("scope:") && part.slice(6).length > 0) {
      tokens.push({ kind: "scope", value: part.slice(6), raw: part });
      continue;
    } else if (part.startsWith("runtime:")) {
      const runtime = part.slice(8);
      if (RUNTIME_COMPAT_KEYS.find(([k]) => runtime == k)) {
        tokens.push({
          kind: `runtimeCompat.${runtime as keyof RuntimeCompat}`,
          value: true,
          raw: part,
        });
        continue;
      }
    }

    tokens.push({ kind: "text", value: part, raw: part });
  }

  return tokens;
}

export function processFilter(
  search: string,
): { query: string; filters: string | undefined } {
  const filters: string[] = [];
  let query = "";
  for (const part of tokenizeFilter(search)) {
    if (part.kind === "text") {
      query += part.value + " ";
    } else if (part.kind === "scope") {
      filters.push(`scope:"${part.value}"`);
    } else {
      // runtimeCompat.<runtime>
      filters.push(`${part.kind}:true`);
    }
  }
  return {
    query: query.trim(),
    filters: filters.length === 0 ? undefined : filters.join(" AND "),
  };
}
