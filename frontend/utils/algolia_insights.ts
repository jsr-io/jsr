// Copyright 2024 the JSR authors. All rights reserved. MIT license.

// Algolia Insights is called directly over its REST API (no SDK) so nothing
// Node-specific ends up in the Cloudflare Worker bundle. All calls are
// best-effort and only run in the browser.
const INSIGHTS_ENDPOINT = "https://insights.algolia.io/1/events";

// Anonymous, stable-per-browser id tying a click to a later conversion.
const USER_TOKEN_KEY = "algolia:userToken";
// A search result click is remembered here so that a later page view on the
// same object (typically the package page) can be attributed as a conversion.
const PENDING_CONVERSION_KEY = "algolia:pendingConversion";
// Only attribute a conversion if it happens within this window of the click.
const PENDING_CONVERSION_TTL_MS = 60 * 60 * 1000; // 1 hour

let appId: string | undefined;
let apiKey: string | undefined;

/** Store the credentials used to send events. Returns whether insights is
 * usable. */
export function initInsights(id: string, key: string): boolean {
  appId = id;
  apiKey = key;
  return true;
}

function userToken(): string {
  try {
    let token = localStorage.getItem(USER_TOKEN_KEY);
    if (!token) {
      token = crypto.randomUUID();
      localStorage.setItem(USER_TOKEN_KEY, token);
    }
    return token;
  } catch {
    return "anonymous";
  }
}

function sendEvents(events: Record<string, unknown>[]): void {
  if (!appId || !apiKey) return;
  try {
    // keepalive lets the request outlive the navigation a click triggers.
    void fetch(INSIGHTS_ENDPOINT, {
      method: "POST",
      headers: {
        "Content-Type": "application/json",
        "X-Algolia-Application-Id": appId,
        "X-Algolia-API-Key": apiKey,
      },
      body: JSON.stringify({ events }),
      keepalive: true,
    }).catch(() => {});
  } catch {
    // best-effort
  }
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
  sendEvents([{
    eventType: "click",
    eventName,
    index,
    userToken: userToken(),
    queryID,
    objectIDs: [objectID],
    positions: [position],
  }]);
  try {
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
  sendEvents([{
    eventType: "conversion",
    eventName,
    index: pending.index,
    userToken: userToken(),
    queryID: pending.queryID,
    objectIDs: [objectID],
  }]);
}
