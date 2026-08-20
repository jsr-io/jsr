// Copyright 2024 the JSR authors. All rights reserved. MIT license.
import type {
  List,
  Package,
  RuntimeCompat,
} from "../frontend/utils/api_types.ts";
import { algoliasearch } from "algoliasearch";

const jsr_url = Deno.env.get("JSR_ENDPOINT_URL");

const client = algoliasearch(
  Deno.env.get("ALGOLIA_APP_ID")!,
  Deno.env.get("ALGOLIA_WRITE_API_KEY")!,
);
const indexName = Deno.env.get("ALGOLIA_PACKAGES_INDEX")!;

export interface AlgoliaPackageHit {
  objectID: string;
  scope: string;
  name: string;
  description: string;
  runtimeCompat: RuntimeCompat;
  score: number | null;
}

// fill the index
let packages: Package[] = [];

let page = 1;
while (true) {
  const packageRes = await fetch(`${jsr_url}/api/packages?page=${page}`);
  const packagesJson: List<Package> = await packageRes.json();

  packages = packages.concat(packagesJson.items);

  if (packagesJson.items.length < 100) {
    break;
  } else {
    page++;
  }
}

const objects = packages
  .filter((entry) =>
    entry.versionCount > 0 || !entry.isArchived ||
    !entry.description.startsWith("INTERNAL")
  )
  .map((entry) => ({
    objectID: `@${entry.scope}/${entry.name}`,
    scope: entry.scope,
    name: entry.name,
    description: entry.description,
    runtimeCompat: entry.runtimeCompat,
    score: entry.score,
  } satisfies AlgoliaPackageHit));

// Index settings (searchable attributes, faceting, ranking) are managed by
// Terraform; replaceAllObjects preserves them across the atomic swap.
await client.replaceAllObjects({ indexName, objects, batchSize: 1000 });
