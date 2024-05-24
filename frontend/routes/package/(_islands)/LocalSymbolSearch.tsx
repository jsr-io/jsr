// Copyright 2024 the JSR authors. All rights reserved. MIT license.
import { JSX } from "preact";
import { computed, Signal, useSignal } from "@preact/signals";
import { useEffect, useRef, useState } from "preact/hooks";
// import {
//   components,
//   create,
//   insertMultiple,
//   type Orama,
//   search,
// } from "@orama/orama";
import { api, path } from "../../../utils/api.ts";
import { useMacLike } from "../../../utils/os.ts";

export interface LocalSymbolSearchProps {
  scope: string;
  pkg: string;
  version: string;
}

interface SearchRecord {
  name: string;
  kind: (
    | "class"
    | "enum"
    | "function"
    | "interface"
    | "namespace"
    | "typeAlias"
    | "variable"
  )[];
  file: string;
  location: { filename: string; line: number; col: number };
}

export function LocalSymbolSearch(
  props: LocalSymbolSearchProps,
) {
  // deno-lint-ignore no-explicit-any
  const db = useSignal<undefined | Orama<any>>(undefined);
  const results = useSignal<SearchRecord[]>([]);
  const selectionIdx = useSignal(-1);
  const macLike = useMacLike();

  useEffect(() => {
    (async () => {
      const [oramaDb, searchResp] = await Promise.all([
        (async () => {
          const tokenizer = await components.tokenizer.createTokenizer();

          return create({
            schema: {
              name: "string",
              kind: "enum[]",
              file: "string",
              location: {
                filename: "string",
                line: "number",
                col: "number",
              },
            },
            components: {
              tokenizer: {
                language: "english",
                normalizationCache: new Map(),
                tokenize(
                  raw: string,
                  lang: string | undefined,
                  prop: string | undefined,
                ) {
                  if (prop === "name") {
                    return raw.split(/(?=[A-Z])/).map((s) => s.toLowerCase());
                  }
                  return tokenizer.tokenize(raw, lang, prop);
                },
              },
            },
          });
        })(),
        api.get<{ nodes: SearchRecord[] }>(
          path`/scopes/${props.scope}/packages/${props.pkg}/versions/${
            props.version || "latest"
          }/docs/search`,
        ),
      ]);

      if (searchResp.ok) {
        // deno-lint-ignore no-explicit-any
        await insertMultiple(oramaDb, searchResp.data.nodes as any);
        db.value = oramaDb;
      } else {
        console.error(searchResp);
      }
    })();
  }, []);

  const ref = useRef<HTMLDivElement>(null);
  const showResults = useSignal(true);
  useEffect(() => {
    const outsideClick = (e: Event) => {
      if (!ref.current) return;
      showResults.value = ref.current.contains(e.target as Element);
    };

    document.addEventListener("click", outsideClick);
    return () => document.removeEventListener("click", outsideClick);
  }, []);

  useEffect(() => {
    const keyboardHandler = (e: KeyboardEvent) => {
      if (((e.metaKey || e.ctrlKey) && e.key === "/")) {
        e.preventDefault();
        (document.querySelector("#symbol-search-input") as HTMLInputElement)
          ?.focus();
      }
    };
    globalThis.addEventListener("keydown", keyboardHandler);
    return function cleanup() {
      globalThis.removeEventListener("keydown", keyboardHandler);
    };
  });

  async function onInput(e: JSX.TargetedEvent<HTMLInputElement>) {
    if (e.currentTarget.value) {
      const searchResult = await search(db.value!, {
        term: e.currentTarget.value,
        properties: ["name"],
        threshold: 0.4,
      });
      selectionIdx.value = -1;
      results.value = searchResult.hits.map((hit) =>
        // deno-lint-ignore no-explicit-any
        hit.document as any as SearchRecord
      );
    } else {
      results.value = [];
    }
  }

  function onKeyUp(e: KeyboardEvent) {
    if (e.key === "ArrowDown") {
      selectionIdx.value = Math.min(
        results.value.length - 1,
        selectionIdx.value + 1,
      );
    } else if (e.key === "ArrowUp") {
      selectionIdx.value = Math.max(0, selectionIdx.value - 1);
    } else if (e.key === "Enter") {
      if (selectionIdx.value > -1) {
        const item = results.value[selectionIdx.value];
        if (item !== undefined) {
          e.preventDefault();
          location.href = `/@${props.scope}/${props.pkg}${
            props.version ? `@${props.version}` : ""
          }/doc${item.file === "." ? "" : item.file}/~/${item.name}`;
        }
      }
    }
  }

  const placeholder = `Search for symbols${
    macLike !== undefined ? ` (${macLike ? "⌘/" : "Ctrl+/"})` : ""
  }`;
  return (
    <div class="flex-none" ref={ref}>
      <input
        type="text"
        placeholder={placeholder}
        id="symbol-search-input"
        class="block text-sm w-full py-2 px-2 input-container input bg-white border-jsr-cyan-300/50"
        disabled={!db}
        onInput={onInput}
        onKeyUp={onKeyUp}
      />
      <div role="listbox" tabindex={0} class="relative">
        {!(!showResults.value || results.value.length == 0) && (
          <ResultList
            results={results}
            searchProps={props}
            selectionIdx={selectionIdx}
          />
        )}
      </div>
    </div>
  );
}

function ResultList(
  { results, searchProps, selectionIdx }: {
    results: Signal<SearchRecord[]>;
    searchProps: LocalSymbolSearchProps;
    selectionIdx: Signal<number>;
  },
) {
  return (
    <div class="absolute md:right-0 bg-white min-w-full border sibling:bg-red-500 shadow z-40">
      <ul class="divide-y-1">
        {results.value.map((result, i) => {
          const selected = computed(() => selectionIdx.value === i);
          return (
            <li
              key={result.file + result.kind + result.name}
              class="hover:bg-gray-100 cursor-pointer aria-[selected=true]:bg-cyan-100"
              aria-selected={selected}
            >
              <a
                href={`/@${searchProps.scope}/${searchProps.pkg}${
                  searchProps.version ? `@${searchProps.version}` : ""
                }/doc${
                  result.file === "." ? "" : result.file
                }/~/${result.name}`}
                class="flex gap-4 items-center justify-between py-2 px-3"
              >
                <div class="flex items-center gap-2.5 ddoc">
                  <div class="flex justify-end compound_types w-[2.125rem] shrink-0">
                    {result.kind.map((kind) => {
                      const [rustKind, title, symbol] =
                        docNodeKindToStringVariants(kind);

                      return (
                        <div
                          class={`text-${rustKind} bg-${rustKind}/15 rounded-full size-5 font-medium text-xs leading-5 text-center align-middle shrink-0 select-none font-mono`}
                          title={title}
                        >
                          {symbol}
                        </div>
                      );
                    })}
                  </div>

                  <span class="text-sm leading-none">
                    {result.name}
                  </span>
                </div>

                <div class="text-xs italic text-stone-400 px-0.5 overflow-hidden whitespace-nowrap text-ellipsis">
                  {result.location.filename}
                </div>
              </a>
            </li>
          );
        })}
      </ul>
    </div>
  );
}

function docNodeKindToStringVariants(kind: string) {
  switch (kind) {
    case "function":
      return ["Function", "Function", "f"];
    case "variable":
      return ["Variable", "Variable", "v"];
    case "class":
      return ["Class", "Class", "c"];
    case "enum":
      return ["Enum", "Enum", "E"];
    case "interface":
      return ["Interface", "Interface", "I"];
    case "typeAlias":
      return ["TypeAlias", "Type Alias", "T"];
    case "namespace":
      return ["Namespace", "Namespace", "N"];
    default:
      return [];
  }
}
