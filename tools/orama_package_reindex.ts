// Copyright 2024 the JSR authors. All rights reserved. MIT license.
import type {
  List,
  Package,
  RuntimeCompat,
} from "../frontend/utils/api_types.ts";
import { OramaCloud } from "@orama/core";
import { chunk } from "@std/collections";

const jsr_url = Deno.env.get("JSR_ENDPOINT_URL");

const orama = new OramaCloud({
  projectId: Deno.env.get("ORAMA_PACKAGE_PROJECT_ID")!,
  apiKey: Deno.env.get("ORAMA_PACKAGE_PROJECT_KEY")!,
});
const datasource = orama.dataSource(Deno.env.get("ORAMA_PACKAGE_DATA_SOURCE")!);

export interface OramaPackageHit {
  id: string;
  scope: string;
  name: string;
  description: string;
  runtimeCompat: RuntimeCompat;
  "_omc:number": number;
  score: number | null;
}

// TODO: clear

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

const entries = packages
  .filter((entry) =>
    entry.versionCount > 0 || !entry.isArchived ||
    !entry.description.startsWith("INTERNAL")
  )
  .map((entry) => ({
    scope: entry.scope,
    name: entry.name,
    description: entry.description,
    runtimeCompat: entry.runtimeCompat,
    score: entry.score,
    "_omc:number": entry.score ?? 0,
    id: `@${entry.scope}/${entry.name}`,
  } satisfies OramaPackageHit));

for (const entriesChunk of chunk(entries, 1000)) {
  await datasource.insertDocuments(entriesChunk);
}
