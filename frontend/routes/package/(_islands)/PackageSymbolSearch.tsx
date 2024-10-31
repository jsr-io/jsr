// Copyright 2024 the JSR authors. All rights reserved. MIT license.
import { JSX } from "preact";
import { useSignal } from "@preact/signals";
import { useEffect, useRef } from "preact/hooks";
import { IS_BROWSER } from "$fresh/runtime.ts";
import {
  components,
  create,
  insertMultiple,
  type Orama,
  search,
} from "@orama/orama";
import { OramaClient } from "npm:@oramacloud/client";
import { Highlight } from "@orama/highlight";
import { api, path } from "../../../utils/api.ts";
import { useMacLike } from "../../../utils/os.ts";

export interface PackageSymbolSearchProps {
  scope: string;
  pkg: string;
  version: string;
  versionIsLatest: boolean;
  content?: string;
  oramaSymbolsIndex: string | undefined;
  oramaSymbolsApiKey: string | undefined;
}

interface SearchItem {
  name: string;
  doc: string;
}

// deno-lint-ignore no-explicit-any
async function createOrama(): Promise<Orama<any>> {
  const tokenizer = await components.tokenizer.createTokenizer();

  return create({
    schema: {
      name: "string",
      doc: "string",
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

function getSearchItemNameAndDescription(searchItem: HTMLElement) {
  const name = (searchItem.getElementsByClassName("namespaceItemContent")[0]
    .children[0] as HTMLAnchorElement).title;
  const description = searchItem.getElementsByClassName(
    "markdown_summary",
  )[0] as HTMLElement | undefined;

  return { name, description };
}

const highlighter = new Highlight();

export function PackageSymbolSearch(
  props: PackageSymbolSearchProps,
) {
  const useCloud = props.versionIsLatest && !!props.oramaSymbolsIndex &&
    !!props.oramaSymbolsApiKey;
  // deno-lint-ignore no-explicit-any
  const db = useSignal<undefined | Orama<any> | OramaClient>(
    useCloud
      ? new OramaClient({
        endpoint: `https://cloud.orama.run/v1/indexes/${props
          .oramaSymbolsIndex!}`,
        api_key: props.oramaSymbolsApiKey!,
      })
      : undefined,
  );
  const showResults = useSignal(false);
  const macLike = useMacLike();
  const searchCounter = useSignal(0);

  useEffect(() => {
    (async () => {
      const [oramaDb, searchResp] = await Promise.all([
        useCloud ? null : createOrama(),
        !props.content
          ? api.get<string>(
            path`/scopes/${props.scope}/packages/${props.pkg}/versions/${
              props.version || "latest"
            }/docs/search_html`,
          )
          : Promise.resolve({ ok: true, data: props.content }),
      ]);

      let searchContent: string;
      if (searchResp.ok) {
        searchContent = searchResp.data;
      } else {
        console.error(searchResp);
        return;
      }

      const searchResults = document.getElementById("docSearchResults")!;
      searchResults.innerHTML = searchContent;

      for (
        const searchItem of searchResults.getElementsByClassName(
          "namespaceItem",
        ) as HTMLCollectionOf<HTMLElement>
      ) {
        searchItem.style.setProperty("display", "none");
        const section = searchItem.parentElement!.parentElement!;
        section.hidden = true;
      }

      if (!useCloud) {
        const searchItems: SearchItem[] = Array.from(
          searchResults
            .getElementsByClassName("namespaceItem") as HTMLCollectionOf<
              HTMLElement
            >,
        )
          .map((searchItem) => {
            const { name, description } = getSearchItemNameAndDescription(
              searchItem,
            );

            return {
              name,
              doc: description?.innerText.replaceAll("\n", " ") ?? "",
            };
          });

        await insertMultiple(oramaDb!, searchItems);
        db.value = oramaDb!;
      }
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

  async function onInput(e: JSX.TargetedEvent<HTMLInputElement>) {
    if (e.currentTarget.value) {
      const term = e.currentTarget.value;

      let searchResult: SearchItem[];

      if (useCloud) {
        searchResult = (await (db.value as OramaClient).search({
          term,
          where: {
            scope: props.scope,
            package: props.pkg,
          },
          mode: "fulltext",
        }))?.hits.map((hit) => hit.document) ?? [];
      } else {
        // deno-lint-ignore no-explicit-any
        searchResult = (await search(db.value! as Orama<any>, {
          term,
          properties: ["name", "doc"],
          threshold: 0.2,
          limit: 50,
        })).hits.map((hit) => hit.document as unknown as SearchItem);
      }

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

      if (searchResult.length === 0) {
        document.getElementById("docSearchResultsEmpty")!.classList.remove(
          "hidden",
        );
      } else {
        document.getElementById("docSearchResultsEmpty")!.classList.add(
          "hidden",
        );

        for (const doc of searchResult) {
          // TODO(@crowlKats): figure out how to handle symbol drilldown for the pre-generated search content page
          const nodes = Array.from(
            document.getElementById("docSearchResults")!.getElementsByClassName(
              "namespaceItem",
            ) as HTMLCollectionOf<HTMLElement>,
          ).filter((node) =>
            getSearchItemNameAndDescription(node).name === doc.name
          );

          for (const node of nodes) {
            const section = node.parentElement!.parentElement!;

            section.hidden = false;
            previousSections.current.add(section);

            node.style.removeProperty("display");
            previousResultNodes.current.push(node);

            const titleElement = node.getElementsByClassName(
              "namespaceItemContent",
            )[0]
              .children[0] as HTMLAnchorElement;
            titleElement.innerHTML =
              highlighter.highlight(titleElement.title, term).HTML;

            const description = node.getElementsByClassName(
              "markdown_summary",
            )[0] as HTMLElement;

            if (description) {
              const positions = highlighter.highlight(doc.doc, term).positions;

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
          }
        }
      }

      searchCounter.value++;
      showResults.value = true;
    } else {
      showResults.value = false;
    }
  }

  if (IS_BROWSER) {
    if (showResults.value && searchCounter.value) {
      document.getElementById("docMain")!.classList.add("hidden");
      document.getElementById("docSearchResults")!.classList.remove("hidden");
      document.getElementById("docSearchResultsOramaLogo")!.classList.remove(
        "hidden",
      );
    } else {
      document.getElementById("docMain")!.classList.remove("hidden");
      document.getElementById("docSearchResults")!.classList.add("hidden");
      document.getElementById("docSearchResultsEmpty")!.classList.add("hidden");
      document.getElementById("docSearchResultsOramaLogo")!.classList.add(
        "hidden",
      );
    }
  }

  const placeholder = `Search for symbols${
    macLike !== undefined ? ` (${macLike ? "⌘/" : "Ctrl+/"})` : ""
  }`;
  return (
    <div class="flex-none">
      <input
        type="search"
        placeholder={placeholder}
        id="symbol-search-input"
        class="block text-sm w-full py-2 px-2 input-container input bg-white border-1 border-jsr-cyan-300/50"
        disabled={!db.value}
        onInput={onInput}
      />
    </div>
  );
}
