// Copyright 2024 the JSR authors. All rights reserved. MIT license.
import type { API } from "./utils/api.ts";
import type { FullUser, RuntimeCompat } from "./utils/api_types.ts";
import type { TraceSpan } from "./utils/tracing.ts";
import type { SourceDir, SourceFile } from "./utils/api_types.ts";
import { createDefine } from "fresh";

export const define = createDefine<State>();

export interface State {
  api: API;
  span: TraceSpan;
  userPromise: Promise<FullUser | null | Response>;
  user: FullUser | null;
  sudo: boolean;
  meta: Meta;
  searchKind?: SearchKind;
}

export interface Meta {
  title?: string;
  description?: string;
  ogImage?: string;
}

export type SearchKind = "packages" | "docs";

export interface Docs {
  css: string;
  comrakCss: string;
  script: string;
  // null only on index page
  breadcrumbs: string | null;
  // null only on all symbols page
  toc: string | null;
  main: string;
}

export interface Source {
  css: string;
  comrakCss: string;
  script: string;
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
