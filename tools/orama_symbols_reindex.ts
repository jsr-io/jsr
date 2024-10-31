// Copyright 2024 the JSR authors. All rights reserved. MIT license.
import type { List, Package } from "../frontend/utils/api_types.ts";
import { chunk } from "jsr:@std/collections";

const index = Deno.env.get("ORAMA_SYMBOLS_INDEX_ID");
const auth = Deno.env.get("ORAMA_PACKAGE_PRIVATE_API_KEY");
const jsr_url = Deno.env.get("JSR_ENDPOINT_URL");

const MAX_ORAMA_INSERT_SIZE = 1000 * 800;

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

let page = 1;
while (true) {
  console.log(`Page: ${page}`);
  const packageRes = await fetch(
    `${jsr_url}/api/packages?page=${page}&limit=10`,
  );
  const packagesJson: List<Package> = await packageRes.json();
  page++;

  for (const pkg of packagesJson.items) {
    if (pkg.latestVersion == null || pkg.isArchived) {
      continue;
    }

    try {
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

      if (strData.length < MAX_ORAMA_INSERT_SIZE) {
        await notify(searchJson.nodes);
      } else {
        const strData = JSON.stringify(searchJson.nodes);
        const chunks = Math.ceil(strData.length / MAX_ORAMA_INSERT_SIZE) * 1.5;
        for (
          const chunkItem of chunk(
            searchJson.nodes,
            Math.floor(searchJson.nodes.length / chunks),
          )
        ) {
          await notify(chunkItem);
        }
      }
    } catch (e) {
      console.log(pkg);
      throw e;
    }
  }

  if (packagesJson.items.length < 10) {
    break;
  }
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

// deno-lint-ignore no-explicit-any
async function notify(chunkItem: any[]) {
  const notifyRes = await fetch(`${ORAMA_URL}/${index}/notify`, {
    method: "POST",
    headers: {
      authorization: `Bearer ${auth}`,
      "Content-Type": "application/json",
    },
    body: JSON.stringify({ "upsert": chunkItem }),
  });
  if (notifyRes.status !== 200) {
    console.log(JSON.stringify(chunkItem).length);
    console.log(await notifyRes.text());
    throw notifyRes;
  }
}
