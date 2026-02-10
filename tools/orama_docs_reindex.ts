// Copyright 2024 the JSR authors. All rights reserved. MIT license.
import { pooledMap } from "@std/async/pool";
import { stripSplitBySections } from "@deno/gfm";
import { extractYaml } from "@std/front-matter";
import GitHubSlugger from "github-slugger";
import { OramaCloud } from "@orama/core";
import TOC from "../frontend/docs/toc.ts";
import { join } from "@std/path";

const orama = new OramaCloud({
  projectId: Deno.env.get("ORAMA_DOCS_PROJECT_ID")!,
  apiKey: Deno.env.get("ORAMA_DOCS_PROJECT_KEY")!,
});
const datasource = orama.dataSource(Deno.env.get("ORAMA_DOCS_DATA_SOURCE")!);

export interface OramaDocsHit {
  path: string;
  header: string;
  headerParts: string[];
  slug: string;
  content: string;
}

const temp = await datasource.createTemporaryIndex();

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
await temp.insertDocuments(entries);
await datasource.swap();