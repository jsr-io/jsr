// Copyright 2024 the JSR authors. All rights reserved. MIT license.
import type { List, Package } from "../frontend/utils/api_types.ts";
import type { OramaPackageHit } from "../frontend/util.ts";

const index = Deno.env.get("ORAMA_PACKAGE_INDEX_ID");
const auth = Deno.env.get("ORAMA_PACKAGE_PRIVATE_API_KEY");
const jsr_url = Deno.env.get("JSR_URL");

const ORAMA_URL = "https://api.oramasearch.com/api/v1/webhooks";

// Clear the index
const res = await fetch(`${ORAMA_URL}/${index}/snapshot`, {
  method: "POST",
  headers: {
    authorization: `Bearer ${auth}`,
    "Content-Type": "application/json",
  },
  body: JSON.stringify([]),
});
if (res.status !== 200) {
  console.log(await res.text());
  throw res;
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

const entries: OramaPackageHit[] = packages
  .filter((entry) => entry.versionCount > 0)
  .map((entry) => ({
    scope: entry.scope,
    name: entry.name,
    description: entry.description,
    runtimeCompat: entry.runtimeCompat,
    score: entry.score,
    id: `@${entry.scope}/${entry.name}`,
  }));

const res2 = await fetch(`${ORAMA_URL}/${index}/notify`, {
  method: "POST",
  headers: {
    authorization: `Bearer ${auth}`,
    "Content-Type": "application/json",
  },
  body: JSON.stringify({ "upsert": entries }),
});
if (res2.status !== 200) {
  console.log(await res2.text());
  throw res2;
}

// deploy the index
const res3 = await fetch(`${ORAMA_URL}/${index}/deploy`, {
  method: "POST",
  headers: {
    authorization: `Bearer ${auth}`,
    "Content-Type": "application/json",
  },
});
if (res3.status !== 200) {
  console.log(await res3.text());
  throw res3;
}
