// Copyright 2024 the JSR authors. All rights reserved. MIT license.
import type { List, Package } from "../frontend/utils/api_types.ts";
import { algoliasearch } from "algoliasearch";

const jsr_url = Deno.env.get("JSR_ENDPOINT_URL");

const client = algoliasearch(
  Deno.env.get("ALGOLIA_APP_ID")!,
  Deno.env.get("ALGOLIA_WRITE_API_KEY")!,
);
const indexName = Deno.env.get("ALGOLIA_SYMBOLS_INDEX")!;

// Index into a temporary index and atomically move it over the live one once
// the full reindex has streamed through, mirroring the previous swap behaviour.
const tmpIndexName = `${indexName}_tmp`;

// Index settings (scope/package faceting) are managed by Terraform on the live
// index. Copy them onto the temp index so the move preserves them.
await client.operationIndex({
  indexName,
  operationIndexParams: {
    operation: "copy",
    destination: tmpIndexName,
    scope: ["settings"],
  },
});

let page = 1;
let objectIdCounter = 0;
while (true) {
  const packageRes = await fetch(
    `${jsr_url}/api/packages?page=${page}&limit=10`,
  );
  const packagesJson: List<Package> = await packageRes.json();
  page++;

  for (const pkg of packagesJson.items) {
    if (pkg.versionCount == 0 || pkg.isArchived) {
      continue;
    }

    const searchRes = await fetch(
      `${jsr_url}/api/scopes/${pkg.scope}/packages/${pkg.name}/versions/${pkg.latestVersion}/docs/search`,
    );
    // deno-lint-ignore no-explicit-any
    const searchJson: any = await searchRes.json();
    if (searchJson.nodes.length === 0) continue;

    for (const node of searchJson.nodes) {
      node.scope = pkg.scope;
      node.package = pkg.name;
      node.objectID = `${pkg.scope}/${pkg.name}/${objectIdCounter++}`;
    }

    await client.saveObjects({
      indexName: tmpIndexName,
      objects: searchJson.nodes,
    });
  }

  if (packagesJson.items.length < 10) {
    break;
  }
}

await client.operationIndex({
  indexName: tmpIndexName,
  operationIndexParams: { operation: "move", destination: indexName },
});
