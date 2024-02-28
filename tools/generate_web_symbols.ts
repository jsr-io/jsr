// Copyright 2024 the JSR authors. All rights reserved. MIT license.
import { Identifier } from "npm:@mdn/browser-compat-data";
import data from "npm:@mdn/browser-compat-data" with { type: "json" };

const webBuiltins: { id: string[]; docs: string }[] = [];

function walk(identifier: Identifier, id: string[] = []) {
  if (
    identifier.__compat?.mdn_url &&
    !(id.at(-1)?.includes("_") ||
      id.at(-1)?.[0].toLowerCase() === id.at(-1)?.[0])
  ) {
    webBuiltins.push({ id, docs: identifier.__compat.mdn_url });
  }
  for (const key in identifier) {
    if (key === "__compat") continue;
    if (
      id.length > 0 &&
      (id.at(-1) === key ||
        key[0].toLowerCase() === key[0] ||
        key.includes("_"))
    ) return;
    walk(identifier[key], [...id, key]);
  }
}

walk(data.javascript.builtins);
walk(data.api);

webBuiltins.push({
  id: ["bigint"],
  docs: data.javascript.builtins.BigInt.__compat!.mdn_url!,
});
webBuiltins.push({
  id: ["boolean"],
  docs: data.javascript.builtins.Boolean.__compat!.mdn_url!,
});
webBuiltins.push({
  id: ["number"],
  docs: data.javascript.builtins.Number.__compat!.mdn_url!,
});
webBuiltins.push({
  id: ["object"],
  docs: data.javascript.builtins.Object.__compat!.mdn_url!,
});
webBuiltins.push({
  id: ["string"],
  docs: data.javascript.builtins.String.__compat!.mdn_url!,
});
webBuiltins.push({
  id: ["symbol"],
  docs: data.javascript.builtins.Symbol.__compat!.mdn_url!,
});
console.log(Object.keys(data.javascript.builtins));

const output = JSON.stringify(webBuiltins, null, 2);
await Deno.writeTextFile("./api/src/docs/web_builtins.json", output);
