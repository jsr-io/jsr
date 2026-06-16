// Copyright 2024 the JSR authors. All rights reserved. MIT license.
import { useEffect } from "preact/hooks";
import { IS_BROWSER } from "fresh/runtime";
import { initInsights, trackResultClick } from "../utils/algolia_insights.ts";

export interface SearchInsightsHit {
  objectID: string;
  /** Path the result links to, e.g. `/@scope/name`. */
  href: string;
  /** 1-based rank within the full result set (across pages). */
  position: number;
}

/**
 * Renders nothing; attaches a delegated click listener that reports clicks on
 * the server-rendered package result list to Algolia Insights. Matching is by
 * link path so it stays decoupled from the list markup.
 */
export default function SearchInsights(
  { appId, apiKey, index, queryID, hits }: {
    appId?: string;
    apiKey?: string;
    index?: string;
    queryID?: string;
    hits: SearchInsightsHit[];
  },
) {
  useEffect(() => {
    if (!IS_BROWSER || !appId || !apiKey || !index || !queryID) return;
    if (!initInsights(appId, apiKey)) return;

    const byPath = new Map(
      hits.map((hit) => [hit.href, hit]),
    );

    const onClick = (e: MouseEvent) => {
      const anchor = (e.target as Element | null)?.closest?.("a");
      if (!anchor) return;
      // The header search (GlobalSearch) tracks its own suggestion clicks; don't
      // also count them here when its results overlap the list.
      if (anchor.closest("#global-search-results")) return;
      const path = new URL(anchor.href, location.origin).pathname;
      const hit = byPath.get(path);
      if (!hit) return;
      trackResultClick({
        index,
        queryID,
        objectID: hit.objectID,
        position: hit.position,
      });
    };

    document.addEventListener("click", onClick, { capture: true });
    return () =>
      document.removeEventListener("click", onClick, { capture: true });
  }, [appId, apiKey, index, queryID]);

  return null;
}
