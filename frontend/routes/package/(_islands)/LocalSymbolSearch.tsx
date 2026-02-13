// Copyright 2024 the JSR authors. All rights reserved. MIT license.
import { JSX } from "preact";
import { createPortal } from "preact/compat";
import { useSignal } from "@preact/signals";
import { useEffect, useRef } from "preact/hooks";
import { IS_BROWSER } from "fresh/runtime";
import {
  components,
  create,
  insertMultiple,
  type Orama,
  search,
} from "@orama/orama";
import { Highlight, type Position } from "@orama/highlight";
import { api, path } from "../../../utils/api.ts";
import { useMacLike } from "../../../utils/os.ts";
import type {
  AllSymbolsCtx,
  AllSymbolsItemCtx,
  SectionContentNamespaceSectionCtx,
} from "@deno/doc/html-types";
import { renderToString } from "preact-render-to-string";
import { AllSymbols } from "../../../components/doc/AllSymbols.tsx";

export interface LocalSymbolSearchProps {
  scope: string;
  pkg: string;
  version: string;
  content?: AllSymbolsCtx;
}

interface SearchItem {
  name: string;
  symbolName: string;
  description: string;
}

// deno-lint-ignore no-explicit-any
async function createOrama(): Promise<Orama<any>> {
  const tokenizer = await components.tokenizer.createTokenizer();

  return create({
    schema: {
      name: "string",
      description: "string",
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
            const tokens = raw.split(/(?=[A-Z])/).map((s) => s.toLowerCase());
            tokens.forEach((token, index) =>
              tokens[index + 1] &&
              tokens.push(token + tokens[index + 1])
            );
            tokens.push(raw.toLowerCase());
            return tokens;
          }
          return tokenizer.tokenize(raw, lang, prop);
        },
      },
    },
  });
}

const highlighter = new Highlight();

function getDocsText(docs: string | null) {
  if (!docs) return "";
  const el = document.createElement("template");
  el.innerHTML = docs;
  return el.content.textContent.replaceAll("\n", " ");
}

export function LocalSymbolSearch(
  props: LocalSymbolSearchProps,
) {
  // deno-lint-ignore no-explicit-any
  const db = useSignal<undefined | Orama<any>>(undefined);
  const showResults = useSignal(false);
  const hasResults = useSignal(true);
  const macLike = useMacLike();
  const searchCounter = useSignal(0);
  const searchContent = useSignal<AllSymbolsCtx | null>(null);

  useEffect(() => {
    (async () => {
      const [oramaDb, searchResp] = await Promise.all([
        createOrama(),
        !props.content
          ? api.get<AllSymbolsCtx>(
            path`/scopes/${props.scope}/packages/${props.pkg}/versions/${
              props.version || "latest"
            }/docs/search_structured`,
          )
          : Promise.resolve({ ok: true, data: props.content }),
      ]);

      if (searchResp.ok) {
        searchContent.value = searchResp.data;
      } else {
        console.error(searchResp);
        return;
      }

      const searchItems: SearchItem[] = [];

      for (const entrypoint of searchContent.value!.entrypoints) {
        for (const kindGroup of entrypoint.module_doc.sections.sections) {
          for (
            const symbol
              of (kindGroup.content as SectionContentNamespaceSectionCtx)
                .content
          ) {
            searchItems.push({
              name: symbol.name,
              symbolName: symbol.name,
              description: getDocsText(symbol.docs),
            });

            for (const subitem of symbol.subitems) {
              searchItems.push({
                name: subitem.title,
                symbolName: symbol.name,
                description: getDocsText(subitem.docs),
              });
            }
          }
        }
      }

      const searchResults = document.getElementById("docSearchResults")!;
      searchResults.innerHTML = renderToString(
        <AllSymbols items={searchContent.value!.entrypoints} />,
      );

      await insertMultiple(oramaDb, searchItems);
      db.value = oramaDb;
    })();
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

  const previousResultNodes = useRef<HTMLElement[]>([]);
  const previousSections = useRef<Set<HTMLElement>>(new Set());
  const searchResultsContainer = useRef<HTMLElement | null>(null);

  // Get the search results container on mount
  useEffect(() => {
    searchResultsContainer.current = document.getElementById(
      "docSearchResults",
    );
  }, []);

  async function onInput(e: JSX.TargetedEvent<HTMLInputElement>) {
    if (e.currentTarget.value) {
      const term = e.currentTarget.value;
      const searchResult = await search(db.value!, {
        term,
        properties: ["name", "description"],
        threshold: 0.2,
        limit: 50,
      });

      for (const node of previousResultNodes.current) {
        node.style.setProperty("display", "none");
        node.querySelectorAll("mark.orama-highlight").forEach((el) => {
          el.replaceWith(...el.childNodes);
        });
        node.normalize();
      }
      previousResultNodes.current = [];

      for (const section of previousSections.current) {
        section.hidden = true;
      }
      previousSections.current.clear();

      const hitNames = new Set(
        searchResult.hits.map((hit) => hit.document.name),
      );

      const out: AllSymbolsItemCtx[] = searchContent.value!.entrypoints
        .map((entrypoint) => {
          const filteredSections = entrypoint.module_doc.sections.sections
            .map((kindGroup) => {
              const filteredContent =
                (kindGroup.content as SectionContentNamespaceSectionCtx).content
                  .map((symbol) => {
                    const symbolMatches = hitNames.has(symbol.name);
                    const matchingSubitems = symbol.subitems.filter((subitem) =>
                      hitNames.has(subitem.title)
                    );

                    if (!symbolMatches && matchingSubitems.length === 0) {
                      return null;
                    }

                    return {
                      ...symbol,
                      subitems: symbolMatches
                        ? symbol.subitems
                        : matchingSubitems,
                    };
                  })
                  .filter(Boolean);

              if (filteredContent.length === 0) return null;

              return {
                ...kindGroup,
                content: { ...kindGroup.content, content: filteredContent },
              };
            })
            .filter(Boolean);

          if (filteredSections.length === 0) return null;

          return {
            ...entrypoint,
            module_doc: {
              ...entrypoint.module_doc,
              sections: {
                ...entrypoint.module_doc.sections,
                sections: filteredSections,
              },
            },
          };
        })
        .filter(Boolean) as AllSymbolsItemCtx[];

      const searchResults = document.getElementById("docSearchResults")!;
      searchResults.innerHTML = highlight(
        renderToString(
          <AllSymbols items={out} />,
        ),
        term,
      );

      hasResults.value = searchResult.hits.length > 0;
      searchCounter.value++;
      showResults.value = true;
    } else {
      hasResults.value = true;
      showResults.value = false;
    }
  }

  if (IS_BROWSER) {
    if (showResults.value && searchCounter.value) {
      document.getElementById("docMain")!.classList.add("hidden");
      document.getElementById("docSearchResults")!.classList.remove("hidden");
    } else {
      document.getElementById("docMain")!.classList.remove("hidden");
      document.getElementById("docSearchResults")!.classList.add("hidden");
    }
  }

  const placeholder = `Search for symbols${
    macLike !== undefined ? ` (${macLike ? "⌘/" : "Ctrl+/"})` : ""
  }`;

  const showNoResults = IS_BROWSER &&
    searchResultsContainer.current &&
    showResults.value &&
    !hasResults.value;

  return (
    <>
      <div class="flex-none">
        <input
          type="search"
          placeholder={placeholder}
          id="symbol-search-input"
          class="block text-sm w-full py-2 px-2 input-container input border-1 border-jsr-cyan-300/50 dark:border-jsr-cyan-800"
          disabled={!db.value}
          onInput={onInput}
        />
      </div>
      {showNoResults &&
        createPortal(
          <div class="text-secondary py-4">No results found</div>,
          searchResultsContainer.current!,
        )}
    </>
  );
}

function highlight(html: string, term: string): string {
  const template = document.createElement("template");
  template.innerHTML = html;
  const fragment = template.content;

  for (const el of fragment.querySelectorAll(".highlightable")) {
    const positions = highlighter.highlight(el.textContent!, term).positions;
    if (positions.length === 0) continue;
    applyHighlights(el, [...positions]);
  }

  const div = document.createElement("div");
  div.appendChild(fragment.cloneNode(true));
  return div.innerHTML;
}

function applyHighlights(fragment: Element, positions: Position[]) {
  if (positions.length > 0) {
    const walker = document.createTreeWalker(
      fragment,
      NodeFilter.SHOW_TEXT,
    );

    let currentPosition = 0;
    let node = walker.nextNode();
    while (node && positions.length) {
      const currentNode = walker.currentNode as Text;
      const textContent = currentNode.textContent!;
      const length = textContent.length;

      const fragments = [];
      let start = 0;

      positionsLoop: for (let i = 0; i < positions.length; i++) {
        const position = positions[i];
        const localStart = position.start - currentPosition;
        const localEnd = position.end - currentPosition;

        if (localStart >= length) {
          // if the start is after the current node, there cannot be more highlights for this node
          break positionsLoop;
        }

        if ((localStart >= 0) && (localEnd < length)) {
          fragments.push(
            textContent.slice(start, localStart),
            textContent.slice(localStart, localEnd + 1),
          );
          start = localEnd + 1;
          positions.shift();
          i--; // we need to recheck the current position
        } else if (localStart >= 0) {
          fragments.push(
            textContent.slice(start, localStart),
            textContent.slice(localStart),
          );
          start = length;
          // if the end is not in this node, there cannot be more highlights for this node
          break positionsLoop;
        } else if (localEnd < length) {
          fragments.push(
            "",
            textContent.slice(start, localEnd + 1),
          );
          start = localEnd + 1;
          positions.shift();
          i--; // we need to recheck the current position
        } else {
          break positionsLoop;
        }
      }

      if (start !== length) {
        fragments.push(textContent.slice(start));
      }

      currentPosition += length;

      node = walker.nextNode();
      if (fragments.length > 1) {
        currentNode.replaceWith(
          document.createRange().createContextualFragment(
            fragments
              .map((fragment, i) =>
                i % 2 === 0
                  ? fragment
                  : fragment !== ""
                  ? `<mark class="orama-highlight">${fragment}</mark>`
                  : ""
              )
              .join(""),
          ),
        );
      }
    }
  }
}
