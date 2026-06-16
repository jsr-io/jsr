// Copyright 2024 the JSR authors. All rights reserved. MIT license.
import { useEffect } from "preact/hooks";
import { IS_BROWSER } from "fresh/runtime";
import { initInsights, trackConversionFor } from "../utils/algolia_insights.ts";

/**
 * Renders nothing; when a package page is opened after a tracked search-result
 * click, reports the view to Algolia Insights as a conversion.
 */
export default function SearchConversion(
  { appId, apiKey, objectID }: {
    appId?: string;
    apiKey?: string;
    objectID: string;
  },
) {
  useEffect(() => {
    if (!IS_BROWSER || !appId || !apiKey) return;
    if (!initInsights(appId, apiKey)) return;
    trackConversionFor(objectID);
  }, [objectID]);

  return null;
}
