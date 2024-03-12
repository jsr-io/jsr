// Copyright 2024 the JSR authors. All rights reserved. MIT license.
import { walk } from "std/fs/walk.ts";
import { pooledMap } from "std/async/pool.ts";
import { stripSplitBySections } from "@deno/gfm";
import { extract } from "std/front_matter/yaml.ts";
import GitHubSlugger from "github-slugger";

const index = Deno.env.get("ORAMA_INDEX_ID");
const auth = Deno.env.get("ORAMA_PRIVATE_API_KEY");

const ORAMA_URL = "https://api.oramasearch.com/api/v1/webhooks";

// Clear the index
await fetch(`${ORAMA_URL}/${index}/snapshot`, {
  method: "POST",
  headers: {
    authorization: `Bearer ${auth}`,
    "Content-Type": "application/json",
  },
  body: JSON.stringify([]),
});

// fill the index
const path = "frontend/docs/";
const results = pooledMap(
  10,
  walk(path, {
    includeDirs: false,
    exts: ["md"],
  }),
  async (entry) => {
    const file = await Deno.readTextFile(entry.path);
    const {
      body,
      attrs,
    } = extract(file);
    const slugger = new GitHubSlugger();

    const sections = stripSplitBySections(body);
    if (
      sections[0].header === "" && sections[0].content !== "" && attrs.title
    ) {
      sections[0].header = attrs.title;
    } else if (sections[0].content === "") {
      sections.shift();
    }

    return sections.map((section) => ({
      path: entry.path.slice(path.length, -3),
      header: section.header,
      slug: slugger.slug(section.header),
      content: section.content,
    }));
  },
);

const entries = (await Array.fromAsync(results)).flat();

await fetch(`${ORAMA_URL}/${index}/notify`, {
  method: "POST",
  headers: {
    authorization: `Bearer ${auth}`,
    "Content-Type": "application/json",
  },
  body: JSON.stringify({"upsert": entries}),
});

// deploy the index
await fetch(`${ORAMA_URL}/${index}/deploy`, {
  method: "POST",
  headers: {
    authorization: `Bearer ${auth}`,
    "Content-Type": "application/json",
  },
});
