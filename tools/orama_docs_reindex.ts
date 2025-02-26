// Copyright 2024 the JSR authors. All rights reserved. MIT license.
import { pooledMap } from "@std/async/pool";
import { stripSplitBySections } from "@deno/gfm";
import { extractYaml } from "@std/front-matter";
import GitHubSlugger from "github-slugger";
import TOC from "../frontend/docs/toc.ts";
import { join } from "@std/path";

const index = Deno.env.get("ORAMA_DOCS_INDEX_ID");
const auth = Deno.env.get("ORAMA_DOCS_PRIVATE_API_KEY");

export interface OramaDocsHit {
  path: string;
  header: string;
  headerParts: string[];
  slug: string;
  content: string;
}

const ORAMA_URL = "https://api.oramasearch.com/api/v1/webhooks";

// Clear the index
const resp1 = await fetch(`${ORAMA_URL}/${index}/snapshot`, {
  method: "POST",
  headers: {
    authorization: `Bearer ${auth}`,
    "Content-Type": "application/json",
  },
  body: JSON.stringify([]),
});
if (!resp1.ok) {
  throw new Error(
    `Failed to clear index: ${resp1.status} ${await resp1.text()}`,
  );
}

// fill the index
const path = "frontend/docs/";
const results = pooledMap(
  10,
  TOC,
  async (entry) => {
    const file = await Deno.readTextFile(
      join(path, entry.id + ".md"),
    );
    const {
      body,
      attrs,
      // deno-lint-ignore no-explicit-any
    } = extractYaml<any>(file);
    const slugger = new GitHubSlugger();

    const sections = stripSplitBySections(body);
    if (sections[0].header === "" && sections[0].content !== "") {
      sections[0].header = attrs.title ?? entry.title;
      sections[0].depth = 1;
    } else if (sections[0].header === "" && sections[0].content === "") {
      sections[0].header = attrs.title ?? entry.title;
      sections[0].content = attrs.description;
      sections[0].depth = 1;
    }

    return sections.map((section, i) => {
      const headerParts: string[] = [section.header];

      let currentDepth = section.depth;
      for (let j = i; currentDepth > 1 && j >= 0; j--) {
        if (sections[j].depth < currentDepth) {
          headerParts.unshift(sections[j].header);
          currentDepth = sections[j].depth;
        }
      }

      return {
        path: entry.id,
        header: section.header,
        headerParts,
        slug: slugger.slug(section.header),
        content: section.content,
      } satisfies OramaDocsHit;
    });
  },
);

const entries = (await Array.fromAsync(results)).flat();

const resp2 = await fetch(`${ORAMA_URL}/${index}/notify`, {
  method: "POST",
  headers: {
    authorization: `Bearer ${auth}`,
    "Content-Type": "application/json",
  },
  body: JSON.stringify({ "upsert": entries }),
});
if (!resp2.ok) {
  throw new Error(
    `Failed to upsert index: ${resp2.status} ${await resp2.text()}`,
  );
}

// deploy the index
const resp3 = await fetch(`${ORAMA_URL}/${index}/deploy`, {
  method: "POST",
  headers: {
    authorization: `Bearer ${auth}`,
    "Content-Type": "application/json",
  },
});
if (!resp3.ok) {
  throw new Error(
    `Failed to deploy index: ${resp3.status} ${await resp3.text()}`,
  );
}
