// Copyright 2024 the JSR authors. All rights reserved. MIT license.
import type { List, Package } from "../frontend/utils/api_types.ts";
import { chunk } from "@std/collections";
import { OramaCloud } from "@orama/core";

const jsr_url = Deno.env.get("JSR_ENDPOINT_URL");

const orama = new OramaCloud({
  projectId: Deno.env.get("ORAMA_SYMBOLS_PROJECT_ID")!,
  apiKey: Deno.env.get("ORAMA_SYMBOLS_PROJECT_KEY")!,
});
const datasource = orama.dataSource(Deno.env.get("ORAMA_SYMBOLS_DATA_SOURCE")!);

const MAX_ORAMA_INSERT_SIZE = 3 * 1024 * 1024;

const temp = await datasource.createTemporaryIndex();

let page = 1;
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
    for (const node of searchJson.nodes) {
      node.scope = pkg.scope;
      node.package = pkg.name;
    }

    const strData = JSON.stringify(searchJson.nodes);
    const chunks = Math.ceil(strData.length / MAX_ORAMA_INSERT_SIZE);

    for (
      const chunkItem of chunk(
        searchJson.nodes,
        searchJson.nodes.length / chunks,
      )
    ) {
      // deno-lint-ignore no-explicit-any
      await temp.insertDocuments(chunkItem as any);
    }
  }

  if (packagesJson.items.length < 10) {
    break;
  }
}

await temp.swap();
