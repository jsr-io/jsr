#!/usr/bin/env -S deno run --allow-net --allow-env
// Copyright 2024 the JSR authors. All rights reserved. MIT license.

import { parse } from "jsr:@std/semver";
import type {
  DependencyGraphItem,
  List,
  Package,
  PackageVersionReference,
} from "../frontend/utils/api_types.ts";

const SOURCE_REGISTRY_URL = Deno.env.get("SOURCE_REGISTRY_URL") ??
  "https://jsr.io";
const LOCAL_REGISTRY_URL = Deno.env.get("LOCAL_REGISTRY_URL") ??
  "http://jsr.test";
const LOCAL_REGISTRY_TOKEN = Deno.env.get("LOCAL_REGISTRY_TOKEN");

if (!LOCAL_REGISTRY_TOKEN) {
  console.error("LOCAL_REGISTRY_TOKEN environment variable is required");
  Deno.exit(1);
}

interface VersionMetadata {
  manifest: Record<string, { size: number; checksum: string }>;
}

interface PackageMeta {
  scope: string;
  name: string;
  latest?: string;
  versions: Record<string, unknown>;
}

async function fetchJson<T>(url: string): Promise<T> {
  const resp = await fetch(url);
  if (!resp.ok) {
    throw new Error(
      `Failed to fetch ${url}: ${resp.status} ${resp.statusText}`,
    );
  }
  return resp.json();
}

interface DependencyInfo {
  deps: Map<string, PackageVersionReference>;
  depGraph: Map<string, string[]>;
}

function pvKey(pv: PackageVersionReference): string {
  return `@${pv.scope}/${pv.package}@${pv.version}`;
}

async function getDependencyInfo(
  scope: string,
  pkg: string,
  version: string,
): Promise<DependencyInfo> {
  const graph = await fetchJson<DependencyGraphItem[]>(
    `${SOURCE_REGISTRY_URL}/api/scopes/${scope}/packages/${pkg}/versions/${version}/dependencies/graph`,
  );

  const idToKey = new Map<number, string>();
  const deps = new Map<string, PackageVersionReference>();

  for (const item of graph) {
    if (item.dependency.type === "jsr") {
      const pv: PackageVersionReference = {
        scope: item.dependency.scope,
        package: item.dependency.package,
        version: item.dependency.version,
      };
      const key = pvKey(pv);
      idToKey.set(item.id, key);
      deps.set(key, pv);
    }
  }

  const rootKey = `@${scope}/${pkg}@${version}`;
  const depGraph = new Map<string, string[]>();
  for (const item of graph) {
    const parentKey = item.dependency.type === "root"
      ? rootKey
      : idToKey.get(item.id);
    if (!parentKey) continue;

    if (!depGraph.has(parentKey)) {
      depGraph.set(parentKey, []);
    }

    for (const childId of item.children) {
      const childKey = idToKey.get(childId);
      if (childKey && childKey !== parentKey) {
        depGraph.get(parentKey)!.push(childKey);
      }
    }
  }

  for (const [key, children] of depGraph) {
    depGraph.set(key, [...new Set(children)]);
  }

  return { deps, depGraph };
}

async function cloneInParallel(
  allDeps: Map<string, PackageVersionReference>,
  depGraph: Map<string, string[]>,
): Promise<void> {
  const inDegree = new Map<string, number>();
  const reverseDeps = new Map<string, string[]>();

  for (const key of allDeps.keys()) {
    const deps = (depGraph.get(key) ?? []).filter((d) => allDeps.has(d));
    inDegree.set(key, deps.length);
    reverseDeps.set(key, []);
  }

  for (const [key, deps] of depGraph) {
    for (const dep of deps) {
      if (allDeps.has(dep)) {
        reverseDeps.get(dep)!.push(key);
      }
    }
  }

  console.log(`\nTotal versions to clone: ${allDeps.size}`);

  const cloned = new Set<string>();
  while (cloned.size < allDeps.size) {
    const ready: string[] = [];
    for (const [key, deg] of inDegree) {
      if (deg === 0 && !cloned.has(key)) {
        ready.push(key);
      }
    }

    console.log(`\nCloning ${ready.length} packages in parallel...`);
    await Promise.all(ready.map(async (key) => {
      await clonePackage(allDeps.get(key)!);
      cloned.add(key);
    }));

    for (const key of ready) {
      for (const dependent of reverseDeps.get(key) ?? []) {
        inDegree.set(dependent, (inDegree.get(dependent) ?? 1) - 1);
      }
      inDegree.delete(key);
    }
  }
}

async function getScopePackages(scope: string): Promise<Package[]> {
  const packages: Package[] = [];
  let page = 1;

  while (true) {
    const url =
      `${SOURCE_REGISTRY_URL}/api/scopes/${scope}/packages?page=${page}&limit=100`;
    const resp = await fetchJson<List<Package>>(url);
    packages.push(...resp.items);
    if (packages.length >= resp.total) break;
    page++;
  }

  return packages;
}

async function ensureScopeExists(scope: string): Promise<void> {
  const url = `${LOCAL_REGISTRY_URL}/api/scopes/${scope}`;
  const resp = await fetch(url, {
    headers: {
      Authorization: `Bearer ${LOCAL_REGISTRY_TOKEN}`,
    },
  });

  if (resp.status === 404) {
    const createResp = await fetch(`${LOCAL_REGISTRY_URL}/api/scopes`, {
      method: "POST",
      headers: {
        "Content-Type": "application/json",
        Authorization: `Bearer ${LOCAL_REGISTRY_TOKEN}`,
      },
      body: JSON.stringify({ scope, description: "" }),
    });

    if (createResp.ok || createResp.status === 409) {
      console.log(`Created scope: ${scope}`);
      return;
    } else {
      throw new Error(`Scope ${scope} is reserved`); // TODO
    }
  } else if (!resp.ok) {
    throw new Error(`Failed to check scope ${scope}: ${resp.status}`);
  }
}

async function ensurePackageExists(scope: string, pkg: string): Promise<void> {
  const url = `${LOCAL_REGISTRY_URL}/api/scopes/${scope}/packages/${pkg}`;
  const resp = await fetch(url, {
    headers: {
      Authorization: `Bearer ${LOCAL_REGISTRY_TOKEN}`,
    },
  });

  if (resp.status === 404) {
    const createResp = await fetch(
      `${LOCAL_REGISTRY_URL}/api/scopes/${scope}/packages`,
      {
        method: "POST",
        headers: {
          "Content-Type": "application/json",
          Authorization: `Bearer ${LOCAL_REGISTRY_TOKEN}`,
        },
        body: JSON.stringify({ package: pkg }),
      },
    );

    if (!createResp.ok && createResp.status !== 409) {
      const text = await createResp.text();
      throw new Error(
        `Failed to create package ${scope}/${pkg}: ${createResp.status} ${text}`,
      );
    }
    console.log(`Created package: @${scope}/${pkg}`);
  } else if (!resp.ok) {
    throw new Error(`Failed to check package ${scope}/${pkg}: ${resp.status}`);
  }
}

async function versionExists(
  scope: string,
  pkg: string,
  version: string,
): Promise<boolean> {
  const url =
    `${LOCAL_REGISTRY_URL}/api/scopes/${scope}/packages/${pkg}/versions/${version}`;
  const resp = await fetch(url, {
    headers: {
      Authorization: `Bearer ${LOCAL_REGISTRY_TOKEN}`,
    },
  });
  return resp.ok;
}

async function clonePackage(pv: PackageVersionReference): Promise<void> {
  const { scope, package: pkg, version } = pv;
  console.log(`\nCloning @${scope}/${pkg}@${version}...`);

  if (await versionExists(scope, pkg, version)) {
    console.log("  Already exists locally, skipping");
    return;
  }

  await ensureScopeExists(scope);
  await ensurePackageExists(scope, pkg);

  console.log("  Fetching metadata...");
  const metadata = await fetchJson<VersionMetadata>(
    `${SOURCE_REGISTRY_URL}/@${scope}/${pkg}/${version}_meta.json`,
  );

  let configFile = "/deno.json";
  for (const path of Object.keys(metadata.manifest)) {
    if (
      path === "/deno.json" ||
      path === "/deno.jsonc" ||
      path === "/jsr.json" ||
      path === "/jsr.jsonc"
    ) {
      configFile = path;
      break;
    }
  }

  const res = await fetch(
    `${SOURCE_REGISTRY_URL}/api/scopes/${scope}/packages/${pkg}/versions/${version}/tarball`,
  );

  console.log("  Publishing...");
  const url =
    `${LOCAL_REGISTRY_URL}/api/scopes/${scope}/packages/${pkg}/versions/${version}?config=${
      encodeURIComponent(configFile)
    }`;

  const resp = await fetch(url, {
    method: "POST",
    headers: {
      "Content-Type": "application/gzip",
      "Content-Encoding": "gzip",
      Authorization: `Bearer ${LOCAL_REGISTRY_TOKEN}`,
    },
    body: res.body!,
  });

  if (!resp.ok) {
    const text = await resp.text();
    throw new Error(
      `Failed to publish @${scope}/${pkg}@${version}: ${resp.status} ${text}`,
    );
  }

  const result = await resp.json();
  console.log(`Published @${scope}/${pkg}@${version} (task: ${result.id})`);

  const maxAttempts = 60;
  for (let i = 0; i < maxAttempts; i++) {
    const resp = await fetch(
      `${LOCAL_REGISTRY_URL}/api/publishing_tasks/${result.id}`,
      {
        headers: {
          Authorization: `Bearer ${LOCAL_REGISTRY_TOKEN}`,
        },
      },
    );

    if (!resp.ok) {
      throw new Error(
        `Failed to get publishing task ${result.id}: ${resp.status}`,
      );
    }

    const task = await resp.json();
    if (task.status === "success") {
      console.log(`  Task ${result.id} completed successfully`);
      return;
    } else if (task.status === "failure") {
      throw new Error(
        `Publishing task ${result.id} failed: ${
          task.error?.message ?? "unknown error"
        }`,
      );
    }

    await new Promise((resolve) => setTimeout(resolve, 1000));
  }

  throw new Error(`Publishing task ${result.id} timed out`);
}

async function cloneScope(scope: string): Promise<void> {
  console.log(`Fetching packages in scope @${scope}...`);
  const packages = await getScopePackages(scope);
  console.log(`Found ${packages.length} packages in @${scope}`);

  const latestVersions: PackageVersionReference[] = [];
  for (const pkg of packages) {
    if (pkg.latestVersion) {
      latestVersions.push({
        scope,
        package: pkg.name,
        version: pkg.latestVersion,
      });
    } else {
      console.log(`  Skipping @${scope}/${pkg.name} (no latest version)`);
    }
  }

  console.log(`\nCollecting ${latestVersions.length} latest versions`);

  console.log(`\nFetching dependency graphs...`);
  const allDeps = new Map<string, PackageVersionReference>();
  const depGraph = new Map<string, string[]>();

  for (const pv of latestVersions) {
    allDeps.set(pvKey(pv), pv);
  }

  const processed = new Set<string>();
  const toProcess = [...latestVersions];

  while (toProcess.length > 0) {
    const pv = toProcess.shift()!;
    const key = pvKey(pv);

    if (processed.has(key)) continue;
    processed.add(key);

    try {
      const info = await getDependencyInfo(pv.scope, pv.package, pv.version);

      const depKeys: string[] = [];
      for (const [depKey, depPv] of info.deps) {
        depKeys.push(depKey);
        if (!allDeps.has(depKey)) {
          allDeps.set(depKey, depPv);
          toProcess.push(depPv);
        }
      }
      depGraph.set(key, depKeys);

      for (const [src, children] of info.depGraph) {
        const existing = depGraph.get(src) ?? [];
        depGraph.set(src, [...new Set([...existing, ...children])]);
      }
    } catch (e) {
      console.log(
        `  Warning: Could not fetch dependency graph for ${key}: ${e}`,
      );
      depGraph.set(key, []);
    }
  }

  await cloneInParallel(allDeps, depGraph);
}

const args = Deno.args;
if (args.length !== 1) {
  console.error("Example: clone_dependency.ts @std/assert@0.215.0");
  console.error("         clone_dependency.ts @std/assert");
  console.error("         clone_dependency.ts @std");
  Deno.exit(1);
}

const input = args[0];

const match = input.match(/^@([^/]+)(?:\/([^@]+)(?:@(.+))?)?$/);
if (!match) {
  console.error(
    `Invalid specifier: ${input}. Expected format: @scope/package@version, @scope/package, or @scope`,
  );
  Deno.exit(1);
}
let [, scope, pkg, version] = match;

if (pkg) {
  if (!version) {
    console.log(`Fetching latest version for @${scope}/${pkg}...`);
    const meta = await fetchJson<PackageMeta>(
      `${SOURCE_REGISTRY_URL}/@${scope}/${pkg}/meta.json`,
    );
    if (!meta.latest) {
      console.error(`No latest version found for @${scope}/${pkg}`);
      Deno.exit(1);
    }
    version = meta.latest;
    console.log(`Using latest version: ${version}`);
  } else {
    try {
      parse(version);
    } catch {
      console.error(`Invalid version: ${version}`);
      Deno.exit(1);
    }
  }

  console.log(`Fetching dependency graph for @${scope}/${pkg}@${version}...`);
  const info = await getDependencyInfo(scope, pkg, version);
  console.log(`Found ${info.deps.size} JSR dependencies`);

  const root: PackageVersionReference = { scope, package: pkg, version };
  info.deps.set(pvKey(root), root);

  await cloneInParallel(info.deps, info.depGraph);
} else {
  await cloneScope(scope);
}

console.log("\nDone!");
