// Copyright 2024 the JSR authors. All rights reserved. MIT license.
import { type JSX } from "preact";
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
import { Highlight } from "@orama/highlight";
import { api, path } from "../../../utils/api.ts";
import { useMacLike } from "../../../utils/os.ts";
import type {
  NamespaceNodeCtx,
  SectionCtx,
  AllSymbolsCtx,
} from "@deno/doc/html-types";
import { Section } from "../../../components/doc/Section.tsx";

export interface LocalSymbolSearchProps {
  scope: string;
  pkg: string;
  version: string;
  content?: AllSymbolsCtx;
}

interface SearchItem {
  name: string;
  description: string;
  sectionIndex: number;
  itemIndex: number;
}

interface NamespaceSectionData {
  sectionIndex: number;
  section: SectionCtx;
  items: NamespaceNodeCtx[];
}

// deno-lint-ignore no-explicit-any
async function createOrama(): Promise<Orama<any>> {
  const tokenizer = await components.tokenizer.createTokenizer();

  return create({
    schema: {
      name: "string",
      description: "string",
      sectionIndex: "number",
      itemIndex: "number",
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

function stripHtml(html: string): string {
  return html.replace(/<[^>]*>/g, "").replace(/\s+/g, " ").trim();
}

const highlighter = new Highlight();

function applyHighlighting(container: HTMLElement, term: string) {
  for (
    const el of container.querySelectorAll(
      ".namespaceItemContent > a",
    ) as NodeListOf<HTMLAnchorElement>
  ) {
    el.innerHTML = highlighter.highlight(el.title, term).HTML;
  }

  for (
    const el of container.querySelectorAll(
      ".namespaceItemContentSubItems > li > a",
    )
  ) {
    el.innerHTML = highlighter.highlight(el.textContent!, term).HTML;
  }

  for (
    const description of container.getElementsByClassName(
      "markdown_summary",
    )
  ) {
    const descText = (description as HTMLElement).innerText.replaceAll(
      "\n",
      " ",
    );
    const positions = highlighter.highlight(descText, term).positions;

    if (positions.length > 0) {
      const walker = document.createTreeWalker(
        description,
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
            break positionsLoop;
          }

          if ((localStart >= 0) && (localEnd < length)) {
            fragments.push(
              textContent.slice(start, localStart),
              textContent.slice(localStart, localEnd + 1),
            );
            start = localEnd + 1;
            positions.shift();
            i--;
          } else if (localStart >= 0) {
            fragments.push(
              textContent.slice(start, localStart),
              textContent.slice(localStart),
            );
            start = length;
            break positionsLoop;
          } else if (localEnd < length) {
            fragments.push(
              "",
              textContent.slice(start, localEnd + 1),
            );
            start = localEnd + 1;
            positions.shift();
            i--;
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
  const resultSections = useSignal<SectionCtx[]>([]);
  const currentTerm = useRef("");
  const namespaceSections = useRef<NamespaceSectionData[]>([]);

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

      let searchContent: AllSymbolsCtx;
      if (searchResp.ok) {
        searchContent = searchResp.data;
      } else {
        console.error(searchResp);
        return;
      }

      const nsSections: NamespaceSectionData[] = [];
      /* TODO searchContent.sections.forEach((section, sectionIndex) => {
        if (section.content.kind === "namespace_section") {
          nsSections.push({
            sectionIndex,
            section,
            items: section.content.content,
          });
        }
      });*/
      namespaceSections.current = nsSections;

      const searchItems: SearchItem[] = nsSections.flatMap(
        (ns) =>
          ns.items.flatMap((item, itemIndex) => {
            const description = item.docs ? stripHtml(item.docs) : "";
            const items: SearchItem[] = [{
              name: item.name,
              description,
              sectionIndex: ns.sectionIndex,
              itemIndex,
            }];

            for (const subitem of item.subitems) {
              items.push({
                name: subitem.title,
                description: "",
                sectionIndex: ns.sectionIndex,
                itemIndex,
              });
            }

            return items;
          }),
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

  const searchResultsContainer = useRef<HTMLElement | null>(null);

  // Get the search results container on mount
  useEffect(() => {
    searchResultsContainer.current = document.getElementById(
      "docSearchResults",
    );
  }, []);

  // Apply highlighting after Preact has committed the DOM
  useEffect(() => {
    const container = searchResultsContainer.current;
    const term = currentTerm.current;
    if (!container || !term || !showResults.value || !hasResults.value) return;
    applyHighlighting(container, term);
  }, [searchCounter.value]);

  async function onInput(e: JSX.TargetedEvent<HTMLInputElement>) {
    if (e.currentTarget.value) {
      const term = e.currentTarget.value;
      currentTerm.current = term;
      const searchResult = await search(db.value!, {
        term,
        properties: ["name", "description"],
        threshold: 0.2,
        limit: 50,
      });

      // Group hits by section, collecting matched item indices
      const sectionMap = new Map<number, Set<number>>();
      for (const hit of searchResult.hits) {
        const doc = hit.document as unknown as SearchItem;
        if (!sectionMap.has(doc.sectionIndex)) {
          sectionMap.set(doc.sectionIndex, new Set());
        }
        sectionMap.get(doc.sectionIndex)!.add(doc.itemIndex);
      }

      // Build filtered SectionCtx[] with only matched items
      const sections: SectionCtx[] = [];
      for (const [sectionIndex, itemIndices] of sectionMap) {
        const ns = namespaceSections.current.find(
          (s) => s.sectionIndex === sectionIndex,
        )!;
        const filteredItems = Array.from(itemIndices).sort().map((i) =>
          ns.items[i]
        );
        sections.push({
          header: ns.section.header,
          content: {
            kind: "namespace_section",
            content: filteredItems,
          },
        });
      }

      resultSections.value = sections;
      hasResults.value = searchResult.hits.length > 0;
      showResults.value = true;
      searchCounter.value++;
    } else {
      hasResults.value = true;
      showResults.value = false;
    }
  }

  if (IS_BROWSER) {
    if (showResults.value) {
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
      {searchResultsContainer.current &&
        showResults.value &&
        hasResults.value &&
        createPortal(
          <div key={searchCounter.value} class="space-y-7">
            {resultSections.value.map((section, i) => (
              <Section key={i} section={section} />
            ))}
          </div>,
          searchResultsContainer.current,
        )}
      {showNoResults &&
        createPortal(
          <div class="text-secondary py-4">No results found</div>,
          searchResultsContainer.current!,
        )}
    </>
  );
}
