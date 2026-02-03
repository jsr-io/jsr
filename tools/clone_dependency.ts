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
import { TarStream, type TarStreamInput } from "jsr:@std/tar";
import type {
  DependencyGraphItem,
  PackageVersionReference,
} from "../frontend/utils/api_types.ts";

const SOURCE_REGISTRY_URL =
  Deno.env.get("SOURCE_REGISTRY_URL") ?? "https://jsr.io";
const LOCAL_REGISTRY_URL =
  Deno.env.get("LOCAL_REGISTRY_URL") ?? "http://jsr.test";
const LOCAL_REGISTRY_TOKEN = Deno.env.get("LOCAL_REGISTRY_TOKEN");

if (!LOCAL_REGISTRY_TOKEN) {
  console.error("LOCAL_REGISTRY_TOKEN environment variable is required");
  Deno.exit(1);
}

interface VersionMetadata {
  manifest: Record<string, { size: number; checksum: string }>;
  exports: Record<string, string>;
  moduleGraph2?: Record<string, unknown>;
}

interface PackageMeta {
  scope: string;
  name: string;
  latest?: string;
  versions: Record<string, unknown>;
}

interface ScopePackage {
  scope: string;
  name: string;
  description: string;
  latestVersion: string | null;
}

// Cache for current user ID
let cachedUserId: string | null = null;

async function getCurrentUserId(): Promise<string> {
  if (cachedUserId) return cachedUserId;

  const resp = await fetch(`${LOCAL_REGISTRY_URL}/api/user`, {
    headers: {
      Authorization: `Bearer ${LOCAL_REGISTRY_TOKEN}`,
    },
  });

  if (!resp.ok) {
    throw new Error(`Failed to get current user: ${resp.status}`);
  }

  const user = await resp.json();
  cachedUserId = user.id;
  return user.id;
}

async function fetchJson<T>(url: string): Promise<T> {
  const resp = await fetch(url);
  if (!resp.ok) {
    throw new Error(`Failed to fetch ${url}: ${resp.status} ${resp.statusText}`);
  }
  return resp.json();
}

async function getDependencyGraph(
  scope: string,
  pkg: string,
  version: string,
): Promise<DependencyGraphItem[]> {
  const url =
    `${SOURCE_REGISTRY_URL}/api/scopes/${scope}/packages/${pkg}/versions/${version}/dependencies/graph`;
  return fetchJson<DependencyGraphItem[]>(url);
}

async function getScopePackages(scope: string): Promise<ScopePackage[]> {
  const packages: ScopePackage[] = [];
  let page = 1;
  const limit = 100;

  while (true) {
    const url =
      `${SOURCE_REGISTRY_URL}/api/scopes/${scope}/packages?page=${page}&limit=${limit}`;
    const resp = await fetchJson<{ items: ScopePackage[]; total: number }>(url);
    packages.push(...resp.items);
    if (packages.length >= resp.total) break;
    page++;
  }

  return packages;
}

async function getPackageMeta(
  scope: string,
  pkg: string,
): Promise<PackageMeta> {
  const url = `${SOURCE_REGISTRY_URL}/@${scope}/${pkg}/meta.json`;
  return fetchJson<PackageMeta>(url);
}

async function getVersionMetadata(
  scope: string,
  pkg: string,
  version: string,
): Promise<VersionMetadata> {
  const url = `${SOURCE_REGISTRY_URL}/@${scope}/${pkg}/${version}_meta.json`;
  return fetchJson<VersionMetadata>(url);
}

async function downloadFile(
  scope: string,
  pkg: string,
  version: string,
  path: string,
): Promise<Uint8Array> {
  const url = `${SOURCE_REGISTRY_URL}/@${scope}/${pkg}/${version}${path}`;
  const resp = await fetch(url);
  if (!resp.ok) {
    throw new Error(
      `Failed to download ${url}: ${resp.status} ${resp.statusText}`,
    );
  }
  return new Uint8Array(await resp.arrayBuffer());
}

function extractJsrDependencies(
  graph: DependencyGraphItem[],
): PackageVersionReference[] {
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

async function createTarball(
  scope: string,
  pkg: string,
  version: string,
  metadata: VersionMetadata,
): Promise<Uint8Array> {
  // Collect all files first
  const files: { path: string; content: Uint8Array }[] = [];

  // Download each file from the manifest
  for (const [path, _entry] of Object.entries(metadata.manifest)) {
    const content = await downloadFile(scope, pkg, version, path);
    const filePath = path.startsWith("/") ? path.slice(1) : path;
    files.push({ path: filePath, content });
  }

  // Create tar stream entries
  const entries: TarStreamInput[] = files.map(({ path, content }) => ({
    type: "file" as const,
    path,
    size: content.length,
    readable: new ReadableStream({
      start(controller) {
        controller.enqueue(content);
        controller.close();
      },
    }),
  }));

  // Create tar stream and collect output
  const tarStream = ReadableStream.from(entries).pipeThrough(new TarStream());
  const reader = tarStream.getReader();
  const chunks: Uint8Array[] = [];

  while (true) {
    const { done, value } = await reader.read();
    if (done) break;
    chunks.push(value);
  }

  // Combine chunks
  const totalLength = chunks.reduce((sum, chunk) => sum + chunk.length, 0);
  const result = new Uint8Array(totalLength);
  let offset = 0;
  for (const chunk of chunks) {
    result.set(chunk, offset);
    offset += chunk.length;
  }

  return result;
}

async function gzipData(data: Uint8Array): Promise<Uint8Array> {
  const stream = new ReadableStream({
    start(controller) {
      controller.enqueue(data);
      controller.close();
    },
  });
  const compressed = stream.pipeThrough(new CompressionStream("gzip"));
  const reader = compressed.getReader();
  const chunks: Uint8Array[] = [];

  while (true) {
    const { done, value } = await reader.read();
    if (done) break;
    chunks.push(value);
  }

  const totalLength = chunks.reduce((sum, chunk) => sum + chunk.length, 0);
  const result = new Uint8Array(totalLength);
  let offset = 0;
  for (const chunk of chunks) {
    result.set(chunk, offset);
    offset += chunk.length;
  }

  return result;
}

async function ensureScopeExists(scope: string): Promise<void> {
  const url = `${LOCAL_REGISTRY_URL}/api/scopes/${scope}`;
  const resp = await fetch(url, {
    headers: {
      Authorization: `Bearer ${LOCAL_REGISTRY_TOKEN}`,
    },
  });

  if (resp.status === 404) {
    // Try creating the scope via regular API first
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
    }

    // If scope is reserved, try admin API
    const errorBody = await createResp.json().catch(() => ({}));
    if (errorBody.code === "scopeNameReserved") {
      const userId = await getCurrentUserId();
      const adminResp = await fetch(`${LOCAL_REGISTRY_URL}/api/admin/scopes`, {
        method: "POST",
        headers: {
          "Content-Type": "application/json",
          Authorization: `Bearer ${LOCAL_REGISTRY_TOKEN}`,
        },
        body: JSON.stringify({ scope, user_id: userId }),
      });

      if (!adminResp.ok && adminResp.status !== 409) {
        const text = await adminResp.text();
        throw new Error(`Failed to create reserved scope ${scope}: ${adminResp.status} ${text}`);
      }
      console.log(`Created reserved scope: ${scope}`);
      return;
    }

    throw new Error(`Failed to create scope ${scope}: ${createResp.status} ${JSON.stringify(errorBody)}`);
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
    // Create the package
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

async function publishVersion(
  scope: string,
  pkg: string,
  version: string,
  tarball: Uint8Array,
  configFile: string,
): Promise<void> {
  const url =
    `${LOCAL_REGISTRY_URL}/api/scopes/${scope}/packages/${pkg}/versions/${version}?config=${encodeURIComponent(configFile)}`;

  const resp = await fetch(url, {
    method: "POST",
    headers: {
      "Content-Type": "application/gzip",
      "Content-Encoding": "gzip",
      Authorization: `Bearer ${LOCAL_REGISTRY_TOKEN}`,
    },
    body: tarball,
  });

  if (!resp.ok) {
    const text = await resp.text();
    throw new Error(
      `Failed to publish @${scope}/${pkg}@${version}: ${resp.status} ${text}`,
    );
  }

  const result = await resp.json();
  console.log(`Published @${scope}/${pkg}@${version} (task: ${result.id})`);

  // Poll for task completion
  await waitForPublishTask(result.id);
}

async function waitForPublishTask(taskId: string): Promise<void> {
  const maxAttempts = 60;
  for (let i = 0; i < maxAttempts; i++) {
    const resp = await fetch(
      `${LOCAL_REGISTRY_URL}/api/publishing_tasks/${taskId}`,
      {
        headers: {
          Authorization: `Bearer ${LOCAL_REGISTRY_TOKEN}`,
        },
      },
    );

    if (!resp.ok) {
      throw new Error(`Failed to get publishing task ${taskId}: ${resp.status}`);
    }

    const task = await resp.json();
    if (task.status === "success") {
      console.log(`  Task ${taskId} completed successfully`);
      return;
    } else if (task.status === "failure") {
      throw new Error(
        `Publishing task ${taskId} failed: ${task.error?.message ?? "unknown error"}`,
      );
    }

    // Still pending/processing, wait and retry
    await new Promise((resolve) => setTimeout(resolve, 1000));
  }

  throw new Error(`Publishing task ${taskId} timed out`);
}

async function clonePackage(pv: PackageVersionReference): Promise<void> {
  const { scope, package: pkg, version } = pv;
  console.log(`\nCloning @${scope}/${pkg}@${version}...`);

  // Check if already exists locally
  if (await versionExists(scope, pkg, version)) {
    console.log(`  Already exists locally, skipping`);
    return;
  }

  // Ensure scope and package exist
  await ensureScopeExists(scope);
  await ensurePackageExists(scope, pkg);

  // Get version metadata
  console.log(`  Fetching metadata...`);
  const metadata = await getVersionMetadata(scope, pkg, version);

  // Determine config file path
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

  // Create tarball
  console.log(`  Creating tarball (${Object.keys(metadata.manifest).length} files)...`);
  const tarData = await createTarball(scope, pkg, version, metadata);
  const gzippedTar = await gzipData(tarData);

  // Publish
  console.log(`  Publishing...`);
  await publishVersion(scope, pkg, version, gzippedTar, configFile);
}

async function cloneScope(scope: string): Promise<void> {
  console.log(`Fetching packages in scope @${scope}...`);
  const packages = await getScopePackages(scope);
  console.log(`Found ${packages.length} packages in @${scope}`);

  // Collect latest version of each package
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

  // Collect all dependencies and build dependency graph for topological sort
  console.log(`\nFetching dependency graphs...`);
  const allDeps = new Map<string, PackageVersionReference>();
  const depGraph = new Map<string, string[]>(); // key -> dependencies

  // Add latest versions to deps map
  for (const pv of latestVersions) {
    const key = `@${pv.scope}/${pv.package}@${pv.version}`;
    allDeps.set(key, pv);
  }

  // Process dependency graphs to find all required versions
  const processed = new Set<string>();
  const toProcess = [...latestVersions];

  while (toProcess.length > 0) {
    const pv = toProcess.shift()!;
    const key = `@${pv.scope}/${pv.package}@${pv.version}`;

    if (processed.has(key)) continue;
    processed.add(key);

    try {
      const graph = await getDependencyGraph(pv.scope, pv.package, pv.version);
      const deps = extractJsrDependencies(graph);
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
      console.log(`  Warning: Could not fetch dependency graph for ${key}: ${e}`);
      depGraph.set(key, []);
    }
  }

  // Topological sort to determine clone order
  const sorted: string[] = [];
  const visited = new Set<string>();
  const visiting = new Set<string>();

  function visit(key: string) {
    if (visited.has(key)) return;
    if (visiting.has(key)) {
      // Circular dependency - just skip (the package will be cloned eventually)
      return;
    }

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

  // Clone in topological order (dependencies first)
  const clonedVersions = new Set<string>();

  for (const key of sorted) {
    if (!clonedVersions.has(key)) {
      const pv = allDeps.get(key)!;
      await clonePackage(pv);
      clonedVersions.add(key);
    }
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

// Check if it's a scope-only input (@scope)
const scopeMatch = input.match(/^@([^/]+)$/);
if (scopeMatch) {
  const [, scope] = scopeMatch;
  await cloneScope(scope);
  console.log("\nDone!");
  Deno.exit(0);
}

let scope: string;
let pkg: string;
let version: string;

// Check if it's @scope/package (without version - use latest)
const packageOnlyMatch = input.match(/^@([^/]+)\/([^@]+)$/);
if (packageOnlyMatch) {
  [, scope, pkg] = packageOnlyMatch;
  console.log(`Fetching latest version for @${scope}/${pkg}...`);
  const meta = await getPackageMeta(scope, pkg);
  if (!meta.latest) {
    console.error(`No latest version found for @${scope}/${pkg}`);
    Deno.exit(1);
  }
  version = meta.latest;
  console.log(`Using latest version: ${version}`);
} else {
  // Otherwise expect @scope/package@version
  const match = input.match(/^@([^/]+)\/([^@]+)@(.+)$/);
  if (!match) {
    console.error(
      `Invalid specifier: ${input}. Expected format: @scope/package@version, @scope/package, or @scope`,
    );
    Deno.exit(1);
  }

  [, scope, pkg, version] = match;

  // Validate version
  try {
    parse(version);
  } catch {
    console.error(`Invalid version: ${version}`);
    Deno.exit(1);
  }
}

console.log(`Fetching dependency graph for @${scope}/${pkg}@${version}...`);
const graph = await getDependencyGraph(scope, pkg, version);

// Extract all JSR dependencies
const deps = extractJsrDependencies(graph);
console.log(`Found ${deps.length} JSR dependencies`);

// Add the main package
deps.push({ scope, package: pkg, version });

// Clone each dependency (they should be in topological order from the API)
for (const dep of deps) {
  await clonePackage(dep);
}

console.log("\nDone!");
