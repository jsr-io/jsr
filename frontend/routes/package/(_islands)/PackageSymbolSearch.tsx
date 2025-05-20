// Copyright 2024 the JSR authors. All rights reserved. MIT license.
import { JSX } from "preact";
import { useSignal, useSignalEffect } from "@preact/signals";
import { useEffect, useMemo, useRef } from "preact/hooks";
import { OramaClient } from "@oramacloud/client";
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
import {
  type CloudSearchItem,
  highlight,
  resetPreviousNodes,
} from "../../../utils/symbolsearch.ts";

export interface LocalSymbolSearchProps {
  scope: string;
  pkg: string;
  version: string;
  isLatestVersion: boolean;
  content?: string;
  indexId?: string;
  apiKey?: string;
}

interface LocalSearchItem {
  name: string;
  description: string;
  node: HTMLElement;
  section: HTMLElement;
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

export function PackageSymbolSearch(
  props: LocalSymbolSearchProps,
) {
  // deno-lint-ignore no-explicit-any
  const db = useSignal<undefined | Orama<any>>(undefined);
  const showResults = useSignal(false);
  const macLike = useMacLike();

  const orama = useMemo(() => {
    if (
      IS_BROWSER && props.indexId && (!props.version || props.isLatestVersion)
    ) {
      return new OramaClient({
        endpoint: `https://cloud.orama.run/v1/indexes/${props.indexId}`,
        api_key: props.apiKey!,
      });
    }
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

  async function onFocus() {
    const searchResults = document.getElementById("docSearchResults")!;

    if (searchResults.innerHTML !== "") {
      return;
    }

    const [localOramaDb, searchResp] = await Promise.all([
      orama ? undefined : createOrama(),
      !props.content
        ? api.get<string>(
          path`/scopes/${props.scope}/packages/${props.pkg}/versions/${props.version}/docs/search_html`,
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

    searchResults.innerHTML = searchContent;

    if (localOramaDb) {
      const searchItems: LocalSearchItem[] = Array.from(
        searchResults
          .getElementsByClassName("namespaceItem") as HTMLCollectionOf<
            HTMLElement
          >,
      )
        .map((searchItem) => {
          const name =
            (searchItem.getElementsByClassName("namespaceItemContent")[0]
              .children[0] as HTMLAnchorElement).title;
          const description = searchItem.getElementsByClassName(
            "markdown_summary",
          )[0] as HTMLElement | undefined;
          searchItem.style.setProperty("display", "none");
          const section = searchItem.parentElement!.parentElement!;
          section.hidden = true;
          return {
            name,
            description: description?.innerText.replaceAll("\n", " ") ?? "",
            node: searchItem,
            section: section,
          };
        });

      await insertMultiple(localOramaDb, searchItems);
      db.value = localOramaDb;
    } else {
      for (
        const searchItem of searchResults
          .getElementsByClassName("namespaceItem") as HTMLCollectionOf<
            HTMLElement
          >
      ) {
        searchItem.style.setProperty("display", "none");
        const section = searchItem.parentElement!.parentElement!;
        section.hidden = true;
      }
    }
  }

  const previousResultNodes = useRef<HTMLElement[]>([]);
  const previousSections = useRef<Set<HTMLElement>>(new Set());

  async function onInput(e: JSX.TargetedEvent<HTMLInputElement>) {
    if (e.currentTarget.value) {
      const term = e.currentTarget.value;

      if (db.value) {
        const searchResult = await search(db.value, {
          term,
          properties: ["name", "description"],
          threshold: 0.2,
          limit: 50,
        });

        resetPreviousNodes(previousResultNodes, previousSections);

        for (const hit of searchResult.hits) {
          const doc = hit.document as unknown as LocalSearchItem;

          highlight(
            highlighter,
            term,
            doc.description,
            doc.section,
            previousSections,
            doc.node,
            previousResultNodes,
          );
        }
      } else {
        const searchResult = await orama!.search({
          term,
          where: {
            scope: props.scope,
            package: props.pkg,
          },
          limit: 50,
          mode: "fulltext",
        });

        resetPreviousNodes(previousResultNodes, previousSections);

        for (const hit of searchResult?.hits ?? []) {
          const doc = hit.document as CloudSearchItem;

          const node = document.getElementById(doc.target_id.toLowerCase())!;
          const section = node.parentElement!.parentElement!;

          highlight(
            highlighter,
            term,
            doc.doc,
            section,
            previousSections,
            node,
            previousResultNodes,
          );
        }
      }

      showResults.value = true;
    } else {
      showResults.value = false;
    }
  }

  useSignalEffect(() => {
    if (showResults.value) {
      document.getElementById("docMain")!.classList.add("hidden");
      document.getElementById("docSearchResults")!.classList.remove("hidden");
    } else {
      document.getElementById("docMain")!.classList.remove("hidden");
      document.getElementById("docSearchResults")!.classList.add("hidden");
    }
  });

  const placeholder = `Search for symbols${
    macLike !== undefined ? ` (${macLike ? "⌘/" : "Ctrl+/"})` : ""
  }`;
  return (
    <div class="flex-none">
      <input
        type="search"
        placeholder={placeholder}
        id="symbol-search-input"
        class="block text-sm w-full py-2 px-2 input-container input border-1 border-jsr-cyan-300/50 dark:border-jsr-cyan-800"
        onInput={onInput}
        onFocus={onFocus}
      />
    </div>
  );
}
