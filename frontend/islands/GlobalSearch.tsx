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

// deno-lint-ignore no-explicit-any
export type WhereClause = Record<string, any>;

function mergeWhere(
  textWhere: WhereClause | undefined,
  uiRuntimes: Set<string>,
  uiScore: number | null,
): WhereClause | undefined {
  const where: WhereClause = { ...textWhere };
  let hasFilters = textWhere !== undefined;

  for (const runtime of uiRuntimes) {
    where[`runtimeCompat.${runtime}`] = true;
    hasFilters = true;
  }

  if (uiScore !== null) {
    where["_omc:number"] = { ...where["_omc:number"], gte: uiScore };
    hasFilters = true;
  }

  return hasFilters ? where : undefined;
}

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

  // UI-driven filter state (separate from text input)
  const uiRuntimes = useSignal<Set<string>>(new Set());
  const uiScore = useSignal<number | null>(null);

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

  function doSearch(textValue: string) {
    if (textValue.length >= 1 || uiRuntimes.value.size > 0 ||
      uiScore.value !== null) {
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
            let term = textValue;
            let where: WhereClause | undefined;
            if (kind === "packages") {
              const parsed = processFilter(textValue);
              term = parsed.query;
              where = mergeWhere(
                parsed.where,
                uiRuntimes.value,
                uiScore.value,
              );
            }
            const res = await orama.search({
              term,
              where,
              limit: 5,
              ...(term ? { mode: "fulltext" as const } : {}),
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
              query: textValue,
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
    search.value = value;
    updateOverlayScroll(ev.currentTarget! as HTMLInputElement);
    doSearch(value);
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

  // Build full search string with UI filters for form submission / navigation
  function buildFullSearch(): string {
    let full = search.value.trim();
    for (const r of uiRuntimes.value) {
      full += ` runtime:${r}`;
    }
    if (uiScore.value !== null) {
      full += ` score:>=${uiScore.value}`;
    }
    return full.trim();
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
        return;
      }
    }

    if (kind === "docs") {
      e.preventDefault();
      return;
    }

    // If UI filters are active, submit with them baked into the search param
    if (uiRuntimes.value.size > 0 || uiScore.value !== null) {
      e.preventDefault();
      const fullSearch = buildFullSearch();
      location.href = `/packages?search=${encodeURIComponent(fullSearch)}`;
    }
  }

  function toggleRuntime(runtime: string) {
    const next = new Set(uiRuntimes.value);
    if (next.has(runtime)) {
      next.delete(runtime);
    } else {
      next.add(runtime);
    }
    uiRuntimes.value = next;
    doSearch(search.value);
  }

  function setScore(value: number | null) {
    uiScore.value = value;
    doSearch(search.value);
  }

  const filterCount = computed(() =>
    uiRuntimes.value.size + (uiScore.value !== null ? 1 : 0)
  );

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
          showFilters={showFilters}
          uiRuntimes={uiRuntimes}
          uiScore={uiScore}
          filterCount={filterCount}
          toggleRuntime={toggleRuntime}
          setScore={setScore}
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
    showFilters,
    uiRuntimes,
    uiScore,
    filterCount,
    toggleRuntime,
    setScore,
  }: Readonly<{
    suggestions: Signal<
      (OramaPackageHit[] | Package[]) | OramaDocsHit[] | null
    >;
    showSuggestions: Signal<boolean>;
    selectionIdx: Signal<number>;
    kind: SearchKind;
    input: Signal<string>;
    showFilters: Signal<boolean>;
    uiRuntimes: Signal<Set<string>>;
    uiScore: Signal<number | null>;
    filterCount: Signal<number>;
    toggleRuntime: (runtime: string) => void;
    setScore: (value: number | null) => void;
  }>,
) {
  if (!showSuggestions.value) return null;

  return (
    <div class="absolute bg-white dark:bg-jsr-gray-950 w-full sibling:bg-red-500 border-1.5 border-jsr-cyan-950 dark:border-jsr-cyan-600 rounded-lg z-40 overflow-hidden top-0.5">
      {suggestions.value === null && kind === "packages"
        ? (
          <div class="bg-white dark:bg-jsr-gray-950 text-jsr-gray-400 dark:text-jsr-gray-500 px-3 py-2 text-xs space-y-1">
            <p>
              Try <code class="search-input-tag">scope:std</code>{" "}
              <code class="search-input-tag">runtime:deno</code>{" "}
              <code class="search-input-tag">{"score:>80"}</code> or use
              Filters below
            </p>
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
          uiRuntimes={uiRuntimes}
          uiScore={uiScore}
          toggleRuntime={toggleRuntime}
          setScore={setScore}
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
                class={`inline-flex items-center gap-1 text-xs transition-colors cursor-pointer ${
                  showFilters.value || filterCount.value > 0
                    ? "text-jsr-cyan-700 dark:text-jsr-cyan-400 font-semibold"
                    : "text-jsr-gray-400 dark:text-jsr-gray-500 hover:text-jsr-cyan-700 dark:hover:text-jsr-cyan-300"
                }`}
              >
                <TbAdjustmentsHorizontal class="size-3.5" />
                Filters{filterCount.value > 0
                  ? ` (${filterCount.value})`
                  : ""}
              </button>
              <a
                class="text-xs text-jsr-gray-400 dark:text-jsr-gray-500 hover:text-jsr-cyan-700 dark:hover:text-jsr-cyan-300 transition-colors"
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
  { uiRuntimes, uiScore, toggleRuntime, setScore }: {
    uiRuntimes: Signal<Set<string>>;
    uiScore: Signal<number | null>;
    toggleRuntime: (runtime: string) => void;
    setScore: (value: number | null) => void;
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
          const active = uiRuntimes.value.has(key);
          return (
            <button
              key={key}
              type="button"
              onClick={(e) => {
                e.preventDefault();
                e.stopPropagation();
                toggleRuntime(key);
              }}
              class={`inline-flex items-center gap-1 text-xs font-semibold px-2 py-0.5 rounded-full border cursor-pointer select-none transition-colors duration-75 ${
                active ? ACTIVE_FILTER_CLASSES : INACTIVE_FILTER_CLASSES
              }`}
            >
              <div
                class="relative h-3 shrink-0"
                style={`aspect-ratio: ${w} / ${h}`}
              >
                <img
                  src={icon}
                  width={w}
                  height={h}
                  alt=""
                  class="h-3 select-none"
                />
              </div>
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
            (value === null && uiScore.value === null) ||
            (value !== null && uiScore.value === value);
          return (
            <button
              key={label}
              type="button"
              onClick={(e) => {
                e.preventDefault();
                e.stopPropagation();
                setScore(value);
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
