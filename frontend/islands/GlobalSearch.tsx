// Copyright 2024 the JSR authors. All rights reserved. MIT license.
import { batch, computed, Signal, useSignal } from "@preact/signals";
import { useEffect, useMemo, useRef } from "preact/hooks";
import { JSX } from "preact/jsx-runtime";
import { OramaCloud } from "@orama/core";
import { Highlight } from "@orama/highlight";
import { IS_BROWSER } from "fresh/runtime";
import type { OramaPackageHit, SearchKind } from "../util.ts";
import { api, path } from "../utils/api.ts";
import type { List, Package, RuntimeCompat } from "../utils/api_types.ts";
import { PackageHit } from "../components/PackageHit.tsx";
import { useMacLike } from "../utils/os.ts";
import type { ListDisplayItem } from "../components/List.tsx";
import { RUNTIME_COMPAT_KEYS } from "../components/RuntimeCompatIndicator.tsx";
import TbAdjustmentsHorizontal from "tb-icons/TbAdjustmentsHorizontal";

interface GlobalSearchProps {
  query?: string;
  projectId?: string;
  apiKey?: string;
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
  <p key="score:">
    Hint: use <code>{"score:>N"}</code> to filter packages by minimum score
  </p>,
];

const SCORE_OPTIONS = [
  { label: "Any", value: null },
  { label: "60+", value: 60 },
  { label: "70+", value: 70 },
  { label: "80+", value: 80 },
  { label: "90+", value: 90 },
] as const;

// The maximum time between a query and the result for that query being
// displayed, if there is a more recent pending query.
const MAX_STALE_RESULT_MS = 200;

export function GlobalSearch(
  {
    query,
    projectId,
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
  const inputOverlayContentRef = useRef<HTMLDivElement>(null);
  const inputOverlayContent2Ref = useRef<HTMLDivElement>(null);
  const sizeClasses = jumbo ? "py-3 px-4 text-lg" : "py-1 px-2 text-base";
  const showFilters = useSignal(false);

  const showSuggestions = computed(() =>
    isFocused.value && (search.value.length > 0 || kind !== "docs")
  );
  const macLike = useMacLike();

  const orama = useMemo(() => {
    if (IS_BROWSER && projectId) {
      return new OramaCloud({
        projectId,
        apiKey: apiKey!,
      });
    }
  }, [projectId, apiKey]);

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

  function triggerSearch(value: string) {
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
            let where: undefined | WhereClause = undefined;
            if (kind === "packages") ({ where, query } = processFilter(value));
            const res = await orama.search({
              term: query,
              where,
              limit: 5,
              mode: "fulltext",
              boost: kind === "packages"
                ? {
                  id: 3,
                  scope: 2,
                  name: 1,
                  description: 0.5,
                }
                : {},
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
              suggestions.value = res?.hits.map((hit) => hit.document) as any ??
                [];
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
  }

  const onInput = (ev: JSX.TargetedEvent<HTMLInputElement>) => {
    const value = ev.currentTarget!.value as string;
    updateOverlayScroll(ev.currentTarget! as HTMLInputElement);
    triggerSearch(value);
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

  function toggleFilter(tokenRaw: string) {
    const tokens = tokenizeFilter(search.value);
    const existing = tokens.find((t) => t.raw === tokenRaw);
    let newValue: string;
    if (existing) {
      newValue = tokens.filter((t) => t !== existing).map((t) => t.raw).join(
        " ",
      );
    } else {
      newValue = (search.value.trim() + " " + tokenRaw).trim();
    }
    triggerSearch(newValue);
  }

  function setScoreFilter(scoreValue: number | null) {
    const tokens = tokenizeFilter(search.value);
    const withoutScore = tokens.filter((t) => t.kind !== "score");
    let newValue = withoutScore.map((t) => t.raw).join(" ");
    if (scoreValue !== null) {
      newValue = (newValue + " score:>=" + scoreValue).trim();
    }
    triggerSearch(newValue);
  }

  const kindPlaceholder = kind === "packages"
    ? "Search for packages"
    : "Search for documentation";
  const placeholder = kindPlaceholder +
    (macLike !== undefined ? ` (${macLike ? "⌘K" : "Ctrl+K"})` : "");

  // Compute active filter state from search text
  const activeFilters = computed(() => {
    const tokens = tokenizeFilter(search.value);
    const runtimes = new Set<string>();
    let scoreValue: number | null = null;
    for (const t of tokens) {
      if (t.kind.startsWith("runtimeCompat.")) {
        runtimes.add(t.kind.slice("runtimeCompat.".length));
      } else if (t.kind === "score") {
        scoreValue = t.value;
      }
    }
    return { runtimes, scoreValue };
  });

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
          showFilters={showFilters}
          activeFilters={activeFilters}
          toggleFilter={toggleFilter}
          setScoreFilter={setScoreFilter}
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
    showFilters,
    activeFilters,
    toggleFilter,
    setScoreFilter,
  }: Readonly<{
    suggestions: Signal<
      (OramaPackageHit[] | Package[]) | OramaDocsHit[] | null
    >;
    showSuggestions: Signal<boolean>;
    selectionIdx: Signal<number>;
    kind: SearchKind;
    input: Signal<string>;
    randomHint: Signal<JSX.Element | null>;
    showFilters: Signal<boolean>;
    activeFilters: Signal<{
      runtimes: Set<string>;
      scoreValue: number | null;
    }>;
    toggleFilter: (tokenRaw: string) => void;
    setScoreFilter: (scoreValue: number | null) => void;
  }>,
) {
  if (!showSuggestions.value) return null;

  const filtersActive = activeFilters.value.runtimes.size > 0 ||
    activeFilters.value.scoreValue !== null;

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
                ? PackageHit(rawHit as (OramaPackageHit | Package))
                : DocsHit(rawHit as OramaDocsHit, input);

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
      {kind === "packages" && showFilters.value && (
        <FilterBar
          activeFilters={activeFilters}
          toggleFilter={toggleFilter}
          setScoreFilter={setScoreFilter}
        />
      )}
      <div class="bg-jsr-cyan-50 dark:bg-jsr-cyan-950/50 flex items-center justify-between py-1.5 px-3 text-sm border-t border-jsr-cyan-100 dark:border-jsr-cyan-900">
        <div class="flex items-center gap-3">
          {kind === "packages" && (
            <>
              <button
                type="button"
                onClick={(e) => {
                  e.preventDefault();
                  e.stopPropagation();
                  showFilters.value = !showFilters.value;
                }}
                class={`flex items-center gap-1 text-xs font-semibold transition-colors ${
                  showFilters.value || filtersActive
                    ? "text-jsr-cyan-800 dark:text-jsr-cyan-300"
                    : "text-jsr-gray-500 dark:text-jsr-gray-400 hover:text-jsr-cyan-700 dark:hover:text-jsr-cyan-300"
                }`}
              >
                <TbAdjustmentsHorizontal class="size-3.5" />
                Filters
                {filtersActive && (
                  <span class="chip bg-jsr-cyan-200 dark:bg-jsr-cyan-900 text-jsr-cyan-800 dark:text-jsr-cyan-200 text-[10px] py-0 px-1.5 leading-[16px]">
                    {activeFilters.value.runtimes.size +
                      (activeFilters.value.scoreValue !== null ? 1 : 0)}
                  </span>
                )}
              </button>
              <a
                class="text-xs text-jsr-gray-500 dark:text-jsr-gray-400 hover:text-jsr-cyan-700 dark:hover:text-jsr-cyan-300 transition-colors"
                href="/docs/faq#can-i-filter-packages-by-compatible-runtime-in-the-search"
                target="_blank"
              >
                Search syntax
              </a>
            </>
          )}
        </div>
        <div class="flex items-center gap-1">
          <span class="text-jsr-gray-400 dark:text-jsr-gray-500 text-xs">
            powered by <span class="sr-only">Orama</span>
          </span>
          <img class="h-3.5 dark:hidden" src="/logos/orama-dark.svg" alt="" />
          <img
            className="h-3.5 hidden dark:block"
            src="/logos/orama-light.svg"
            alt=""
          />
        </div>
      </div>
    </div>
  );
}

const ACTIVE_FILTER_CLASSES =
  "border-jsr-cyan-300 dark:border-jsr-cyan-700 bg-jsr-cyan-100 dark:bg-jsr-cyan-900 text-jsr-cyan-800 dark:text-jsr-cyan-200";
const INACTIVE_FILTER_CLASSES =
  "border-jsr-gray-200 dark:border-jsr-gray-700 text-jsr-gray-600 dark:text-jsr-gray-300 hover:bg-jsr-cyan-50 dark:hover:bg-jsr-cyan-950 hover:border-jsr-cyan-200 dark:hover:border-jsr-cyan-800";

function FilterBar(
  { activeFilters, toggleFilter, setScoreFilter }: {
    activeFilters: Signal<{
      runtimes: Set<string>;
      scoreValue: number | null;
    }>;
    toggleFilter: (tokenRaw: string) => void;
    setScoreFilter: (scoreValue: number | null) => void;
  },
) {
  return (
    <div
      class="px-3 py-2.5 border-t border-jsr-cyan-100 dark:border-jsr-cyan-900 space-y-2.5"
      onClick={(e) => e.stopPropagation()}
    >
      <div class="flex flex-wrap items-center gap-1.5">
        <span class="text-xs text-jsr-gray-500 dark:text-jsr-gray-400 font-semibold mr-0.5 select-none">
          Runtime
        </span>
        {RUNTIME_COMPAT_KEYS.map(([key, name, icon, w, h]) => {
          const active = activeFilters.value.runtimes.has(key);
          return (
            <button
              key={key}
              type="button"
              onClick={(e) => {
                e.preventDefault();
                e.stopPropagation();
                toggleFilter(`runtime:${key}`);
              }}
              class={`inline-flex items-center gap-1 text-xs font-semibold px-2 py-0.5 rounded-full border cursor-pointer select-none transition-colors duration-75 ${
                active ? ACTIVE_FILTER_CLASSES : INACTIVE_FILTER_CLASSES
              }`}
            >
              <img
                src={icon}
                width={w}
                height={h}
                alt=""
                class="h-3"
                style={`aspect-ratio: ${w} / ${h}`}
              />
              {name}
            </button>
          );
        })}
      </div>
      <div class="flex flex-wrap items-center gap-1.5">
        <span class="text-xs text-jsr-gray-500 dark:text-jsr-gray-400 font-semibold mr-0.5 select-none">
          Min score
        </span>
        {SCORE_OPTIONS.map(({ label, value }) => {
          const active =
            (value === null && activeFilters.value.scoreValue === null) ||
            (value !== null && activeFilters.value.scoreValue === value);
          return (
            <button
              key={label}
              type="button"
              onClick={(e) => {
                e.preventDefault();
                e.stopPropagation();
                setScoreFilter(value);
              }}
              class={`text-xs font-semibold px-2 py-0.5 rounded-full border cursor-pointer select-none transition-colors duration-75 ${
                active ? ACTIVE_FILTER_CLASSES : INACTIVE_FILTER_CLASSES
              }`}
            >
              {label}
            </button>
          );
        })}
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
export type ScoreOp = "gt" | "gte" | "lt" | "lte";
interface ScoreToken {
  kind: "score";
  op: ScoreOp;
  value: number;
  raw: string;
}

type Token = TextToken | ScopeToken | RuntimeToken | ScoreToken;

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
    } else if (part.startsWith("score:")) {
      const rest = part.slice(6);
      const match = rest.match(/^(>=|<=|>|<)(\d+)$/);
      if (match) {
        const opMap: Record<string, ScoreOp> = {
          ">": "gt",
          ">=": "gte",
          "<": "lt",
          "<=": "lte",
        };
        tokens.push({
          kind: "score",
          op: opMap[match[1]],
          value: parseInt(match[2], 10),
          raw: part,
        });
        continue;
      }
    }

    tokens.push({ kind: "text", value: part, raw: part });
  }

  return tokens;
}

// deno-lint-ignore no-explicit-any
export type WhereClause = Record<string, any>;

export function processFilter(
  search: string,
): { query: string; where: WhereClause | undefined } {
  const where: WhereClause = {};
  let query = "";
  let hasFilters = false;
  for (const part of tokenizeFilter(search)) {
    if (part.kind === "text") {
      query += part.value + " ";
    } else if (part.kind === "score") {
      where["_omc:number"] = {
        ...where["_omc:number"],
        [part.op]: part.value,
      };
      hasFilters = true;
    } else {
      where[part.kind] = part.value;
      hasFilters = true;
    }
  }
  return {
    query: query.trim(),
    where: hasFilters ? where : undefined,
  };
}
