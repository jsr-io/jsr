// Copyright 2024 the JSR authors. All rights reserved. MIT license.
import { doc } from "@deno/doc";
import type { DocNode } from "@deno/doc";

const symbols: Set<string> = new Set();

const output = await (new Deno.Command(Deno.execPath(), {
  args: ["types", "--unstable"],
})).output();

const docNodes = await doc("asset:types.ts", {
  // deno-lint-ignore require-await
  async load(specifier: string) {
    return {
      kind: "module",
      specifier,
      content: output.stdout,
    };
  },
});

function getNodeName(docNode: DocNode, base?: string) {
  const name = base ? `${base}.${docNode.name}` : docNode.name;
  symbols.add(name);
  if (docNode.kind === "namespace") {
    for (const subDocNode of docNode.namespaceDef.elements) {
      getNodeName(subDocNode, name);
    }
  }
}

for (const docNode of docNodes) {
  getNodeName(docNode);
}

const outSymbols: Array<Array<string>> = [];

for (const symbol of symbols) {
  outSymbols.push(symbol.split("."));
}

await Deno.writeTextFile(
  "./api/src/docs/deno_types.json",
  JSON.stringify(outSymbols, null, 2),
);
