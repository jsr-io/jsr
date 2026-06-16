// Copyright 2024 the JSR authors. All rights reserved. MIT license.
import aa from "search-insights";

// A search result click is remembered here so that a later page view on the
// same object (typically the package page) can be attributed back to the query
// as a conversion.
const PENDING_CONVERSION_KEY = "algolia:pendingConversion";
// Only attribute a conversion if it happens within this window of the click.
const PENDING_CONVERSION_TTL_MS = 60 * 60 * 1000; // 1 hour

let initialized = false;

/** Initialize search-insights once. Returns whether insights is usable. */
export function initInsights(appId: string, apiKey: string): boolean {
  if (initialized) return true;
  try {
    // `useCookie` persists an anonymous user token so a click and a later
    // conversion on another page share the same token (required for
    // attribution). The token is a random id, not personal data.
    aa("init", { appId, apiKey, useCookie: true });
    initialized = true;
  } catch {
    // search-insights unavailable or blocked; analytics is best-effort.
  }
  return initialized;
}

interface PendingConversion {
  index: string;
  queryID: string;
  objectID: string;
  ts: number;
}

/**
 * Record a click on a search result, and remember it so a later view of the
 * same object can be counted as a conversion. `position` is the 1-based rank of
 * the hit within the full result set.
 */
export function trackResultClick(
  { index, queryID, objectID, position, eventName = "Search Result Clicked" }: {
    index: string;
    queryID: string;
    objectID: string;
    position: number;
    eventName?: string;
  },
) {
  if (!queryID || !objectID) return;
  try {
    aa("clickedObjectIDsAfterSearch", {
      eventName,
      index,
      queryID,
      objectIDs: [objectID],
      positions: [position],
    });
    const pending: PendingConversion = {
      index,
      queryID,
      objectID,
      ts: Date.now(),
    };
    sessionStorage.setItem(PENDING_CONVERSION_KEY, JSON.stringify(pending));
  } catch {
    // ignore
  }
}

/**
 * Fire a conversion for `objectID` if it matches a recent tracked click. The
 * pending click is consumed (one-shot) so a refresh can't double-count.
 */
export function trackConversionFor(
  objectID: string,
  eventName = "Package Viewed After Search",
) {
  let pending: PendingConversion;
  try {
    const raw = sessionStorage.getItem(PENDING_CONVERSION_KEY);
    if (!raw) return;
    pending = JSON.parse(raw) as PendingConversion;
  } catch {
    return;
  }
  if (pending.objectID !== objectID) return;
  sessionStorage.removeItem(PENDING_CONVERSION_KEY);
  if (Date.now() - pending.ts > PENDING_CONVERSION_TTL_MS) return;
  try {
    aa("convertedObjectIDsAfterSearch", {
      eventName,
      index: pending.index,
      queryID: pending.queryID,
      objectIDs: [objectID],
    });
  } catch {
    // ignore
  }
}
