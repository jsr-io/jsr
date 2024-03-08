// Copyright 2024 the JSR authors. All rights reserved. MIT license.
import { API } from "./utils/api.ts";
import type { FullUser, RuntimeCompat } from "./utils/api_types.ts";
import type { TraceSpan } from "./utils/tracing.ts";
import { SourceDir, SourceFile } from "./utils/api_types.ts";

export interface State {
  api: API;
  span: TraceSpan;
  userPromise: Promise<FullUser | null | Response>;
  user: FullUser | null;
}

export interface Docs {
  css: string;
  script: string;
  // null only on index page
  breadcrumbs: string | null;
  // null only on all symbols page
  sidepanel: string | null;
  main: string;
}

export interface Source {
  css: string;
  source: SourceDir | SourceFile;
}

export interface OramaPackageHit {
  id: string;
  scope: string;
  name: string;
  description: string;
  runtimeCompat: RuntimeCompat;
  score: number | null;
}

export interface PaginationData {
  page: number;
  limit: number;
  total: number;
}
