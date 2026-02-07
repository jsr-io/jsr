#!/usr/bin/env -S deno run --allow-net --allow-env

/**
 * Clone a package and its dependencies from jsr.io to a local JSR instance.
 *
 * Usage:
 *   deno run --allow-net --allow-env tools/clone_dependency.ts @scope/package@version
 *   deno run --allow-net --allow-env tools/clone_dependency.ts @scope/package
 *   deno run --allow-net --allow-env tools/clone_dependency.ts @scope
 *
 * Examples:
 *   Clone a single package version with dependencies:
 *     tools/clone_dependency.ts @std/assert@0.215.0
 *
 *   Clone the latest version of a package with dependencies:
 *     tools/clone_dependency.ts @std/assert
 *
 *   Clone an entire scope (all packages and versions):
 *     tools/clone_dependency.ts @std
 *
 * Environment variables:
 *   LOCAL_REGISTRY_URL - URL of the local registry (default: http://jsr.test)
 *   LOCAL_REGISTRY_TOKEN - Auth token for the local registry
 *   SOURCE_REGISTRY_URL - URL of the source registry (default: https://jsr.io)
 */

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

async function getDependencies(
  scope: string,
  pkg: string,
  version: string,
): Promise<PackageVersionReference[]> {
  const graph = await fetchJson<DependencyGraphItem[]>(`${SOURCE_REGISTRY_URL}/api/scopes/${scope}/packages/${pkg}/versions/${version}/dependencies/graph`);

  const seen = new Set<string>();
  const deps: PackageVersionReference[] = [];

  for (const item of graph) {
    if (item.dependency.type === "jsr") {
      const key =
        `@${item.dependency.scope}/${item.dependency.package}@${item.dependency.version}`;
      if (!seen.has(key)) {
        seen.add(key);
        deps.push({
          scope: item.dependency.scope,
          package: item.dependency.package,
          version: item.dependency.version,
        });
      }
    }
  }

  return deps;
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
  const depGraph = new Map<string, string[]>(); // key -> dependencies

  for (const pv of latestVersions) {
    const key = `@${pv.scope}/${pv.package}@${pv.version}`;
    allDeps.set(key, pv);
  }

  const processed = new Set<string>();
  const toProcess = [...latestVersions];

  while (toProcess.length > 0) {
    const pv = toProcess.shift()!;
    const key = `@${pv.scope}/${pv.package}@${pv.version}`;

    if (processed.has(key)) continue;
    processed.add(key);

    try {
      const deps = await getDependencies(pv.scope, pv.package, pv.version);
      const depKeys: string[] = [];

      for (const dep of deps) {
        const depKey = `@${dep.scope}/${dep.package}@${dep.version}`;
        depKeys.push(depKey);
        if (!allDeps.has(depKey)) {
          allDeps.set(depKey, dep);
          toProcess.push(dep);
        }
      }

      depGraph.set(key, depKeys);
    } catch (e) {
      console.log(
        `  Warning: Could not fetch dependency graph for ${key}: ${e}`,
      );
      depGraph.set(key, []);
    }
  }

  const sorted: string[] = [];
  const visited = new Set<string>();
  const visiting = new Set<string>();

  function visit(key: string) {
    if (visited.has(key) || visiting.has(key)) return;

    visiting.add(key);
    const deps = depGraph.get(key) ?? [];
    for (const dep of deps) {
      visit(dep);
    }
    visiting.delete(key);
    visited.add(key);
    sorted.push(key);
  }

  for (const key of allDeps.keys()) {
    visit(key);
  }

  console.log(`\nTotal versions to clone: ${sorted.length}`);

  for (const key of sorted) {
    const pv = allDeps.get(key)!;
    await clonePackage(pv);
  }
}

const args = Deno.args;
if (args.length !== 1) {
  console.error("Usage: clone_dependency.ts @scope/package@version");
  console.error("       clone_dependency.ts @scope/package");
  console.error("       clone_dependency.ts @scope");
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
  const deps = await getDependencies(scope, pkg, version);
  console.log(`Found ${deps.length} JSR dependencies`);

  deps.push({ scope, package: pkg, version });

  for (const dep of deps) {
    await clonePackage(dep);
  }
} else {
  await cloneScope(scope);
}

console.log("\nDone!");
