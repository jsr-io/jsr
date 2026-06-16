// Copyright 2024 the JSR authors. All rights reserved. MIT license.
import { pooledMap } from "@std/async/pool";
import { stripSplitBySections } from "@deno/gfm";
import { extractYaml } from "@std/front-matter";
import GitHubSlugger from "github-slugger";
import { algoliasearch } from "algoliasearch";
import TOC from "../frontend/docs/toc.ts";
import { join } from "@std/path";

const client = algoliasearch(
  Deno.env.get("ALGOLIA_APP_ID")!,
  Deno.env.get("ALGOLIA_WRITE_API_KEY")!,
);
const indexName = Deno.env.get("ALGOLIA_DOCS_INDEX")!;

export interface AlgoliaDocsHit {
  objectID: string;
  path: string;
  header: string;
  headerParts: string[];
  slug: string;
  content: string;
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

      const slug = slugger.slug(section.header);

      return {
        objectID: `${entry.id}#${slug}`,
        path: entry.id,
        header: section.header,
        headerParts,
        slug,
        content: section.content,
      } satisfies AlgoliaDocsHit;
    });
  },
);

const objects = (await Array.fromAsync(results)).flat();

// Index settings (searchable attributes) are managed by Terraform;
// replaceAllObjects preserves them across the atomic swap.
await client.replaceAllObjects({ indexName, objects, batchSize: 1000 });
