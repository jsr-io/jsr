// Copyright 2024 the JSR authors. All rights reserved. MIT license.
import { Client } from "postgres";
import type { DocNode, JsDocTagDoc } from "@deno/doc";

const client = new Client(
  "postgres://postgres:postgres@127.0.0.1:5432/registry3",
);
await client.connect();

const packages = await client.queryObject<
  {
    version: string;
    scope: string;
    name: string;
    exports: Record<string, string>;
  }
>(
  "SELECT version, scope, name, exports FROM package_versions",
);

await Promise.all(packages.rows.map((row) => generateMeta(row)));

async function generateMeta(
  { version, scope, name, exports }: {
    version: string;
    scope: string;
    name: string;
    exports: Record<string, string>;
  },
) {
  const mainEntrypoint: string | undefined = exports["."]
    ? (new URL(exports["."], "file:///")).href
    : undefined;
  const readme = await getReadme(version, scope, name);
  const docNodes = await getDocNodes(version, scope, name);
  const mainEntrypointDoc = mainEntrypoint
    ? docNodes[mainEntrypoint].find((node) => node.kind === "moduleDoc")?.jsDoc
    : undefined;

  const meta = {
    hasReadme: !!readme ||
      (!!mainEntrypointDoc?.doc && mainEntrypointDoc.doc.length != 0),
    hasReadmeExamples: readme
      ? readme.includes("```")
      : !!((mainEntrypointDoc?.tags?.find((tag) => tag.kind === "example") as
        | JsDocTagDoc
        | undefined)?.doc?.includes?.("```")),
    percentageDocumentedSymbols: percentageOfSymbolsWithDocs(docNodes),
    allEntrypointsDocs: allEntrypointsHaveModuleDoc(docNodes),
    allFastCheck: true,
  };

  await client.queryObject(
    `UPDATE package_versions SET meta = '${
      JSON.stringify(meta)
    }' WHERE scope = '${scope}' AND name = '${name}' AND version = '${version}'`,
  );
}

async function getReadme(
  version: string,
  scope: string,
  name: string,
): Promise<null | string> {
  const readme = await client.queryObject<{ path: string }>(
    `SELECT path FROM package_files WHERE scope = '${scope}' AND name = '${name}' AND version = '${version}' AND path ILIKE '/README%'`,
  );

  if (readme.rows.length === 0) {
    return null;
  }

  return await Deno.readTextFile(
    `./.gcs/modules/@${scope}/${name}/${version}${readme.rows[0].path}`,
  );
}

async function getDocNodes(
  version: string,
  scope: string,
  name: string,
): Promise<Record<string, DocNode[]>> {
  const res = await Deno.readTextFile(
    `./.gcs/docs/@${scope}/${name}/${version}/raw.json`,
  );
  return JSON.parse(res);
}

function allEntrypointsHaveModuleDoc(docNodes: Record<string, DocNode[]>) {
  modules: for (const [_specifier, nodes] of Object.entries(docNodes)) {
    for (const node of nodes) {
      if (node.kind == "moduleDoc") {
        continue modules;
      }
    }

    return false;
  }

  return true;
}

function percentageOfSymbolsWithDocs(docNodes: Record<string, DocNode[]>) {
  let totalSymbols = 0;
  let documentedSymbols = 0;

  for (const [_specifier, nodes] of Object.entries(docNodes)) {
    for (const node of nodes) {
      if (
        node.kind == "moduleDoc" ||
        node.kind == "import" ||
        node.declarationKind == "private"
      ) {
        continue;
      }

      totalSymbols += 1;

      if (node.jsDoc) {
        documentedSymbols += 1;
      }
    }
  }

  if (totalSymbols === 0) {
    return 1;
  }

  return documentedSymbols / totalSymbols;
}
