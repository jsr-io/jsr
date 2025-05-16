// Copyright 2024 the JSR authors. All rights reserved. MIT license.
import { JSX } from "preact";
import { useSignal, useSignalEffect } from "@preact/signals";
import { useEffect, useMemo, useRef } from "preact/hooks";
import { OramaClient } from "@oramacloud/client";
import { IS_BROWSER } from "fresh/runtime";
import { Highlight } from "@orama/highlight";
import { api, path } from "../../../utils/api.ts";
import { useMacLike } from "../../../utils/os.ts";
import {
  CloudSearchItem,
  highlight,
  resetPreviousNodes,
} from "../../../utils/symbolsearch.ts";

export interface LocalSymbolSearchProps {
  scope: string;
  indexId?: string;
  apiKey?: string;
}

const highlighter = new Highlight();

export function ScopeSymbolSearch(
  props: LocalSymbolSearchProps,
) {
  const showResults = useSignal(false);
  const macLike = useMacLike();

  const orama = useMemo(() => {
    if (IS_BROWSER && props.indexId) {
      return new OramaClient({
        endpoint: `https://cloud.orama.run/v1/indexes/${props.indexId}`,
        api_key: props.apiKey!,
      });
    } else {
      return;
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

  const previousResultNodes = useRef<HTMLElement[]>([]);
  const previousSections = useRef<Set<HTMLElement>>(new Set());

  async function onFocus() {
    const searchResults = document.getElementById("docSearchResults")!;

    if (searchResults.innerHTML !== "") {
      return;
    }

    const searchResp = await api.get<string>(
      path`/scopes/${props.scope}/search_html`,
    );

    let searchContent: string;
    if (searchResp.ok) {
      searchContent = searchResp.data;
    } else {
      console.error(searchResp);
      return;
    }

    searchResults.innerHTML = searchContent;

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

  async function onInput(e: JSX.TargetedEvent<HTMLInputElement>) {
    if (e.currentTarget.value) {
      const term = e.currentTarget.value;

      const searchResult = await orama!.search({
        term,
        where: {
          scope: props.scope,
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

      showResults.value = true;
    } else {
      showResults.value = false;
    }
  }

  useSignalEffect(() => {
    if (showResults.value) {
      document.getElementById("packageList")!.classList.add("hidden");
      document.getElementById("docSearchResults")!.classList.remove("hidden");
    } else {
      document.getElementById("packageList")!.classList.remove("hidden");
      document.getElementById("docSearchResults")!.classList.add("hidden");
    }
  });

  if (!props.indexId) {
    return null;
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
        class="block text-sm w-full py-2 px-2 input-container input border-1 border-jsr-cyan-300/50 dark:border-jsr-cyan-800"
        disabled={!orama}
        onInput={onInput}
        onFocus={onFocus}
      />
    </div>
  );
}
