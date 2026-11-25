#!/usr/bin/env -S deno run -A
// Copyright 2024 the JSR authors. All rights reserved. MIT license.

// Runs the API worker under `wrangler dev` against a local Postgres and checks
// the paths that only exist on the worker: the request and response bridge,
// the Hyperdrive socket and the R2 bindings.

import { Client } from "postgres";

const ADMIN_DATABASE_URL = Deno.env.get("DATABASE_URL") ??
  "postgres://user:password@127.0.0.1:5432/registry";
const DATABASE = "registry_worker_smoke";
const PORT = 8788;
const API = `http://127.0.0.1:${PORT}/api`;
const TOKEN = "worker-smoke-token";
const SERVICE_ACCOUNT_ID = "00000000-0000-0000-0000-000000000000";

const root = new URL("../", import.meta.url);

function assertEquals<T>(actual: T, expected: T, what: string) {
  if (actual !== expected) {
    throw new Error(`${what}: expected ${expected}, got ${actual}`);
  }
}

async function sha256Hex(data: string): Promise<string> {
  const digest = await crypto.subtle.digest(
    "SHA-256",
    new TextEncoder().encode(data),
  );
  return Array.from(new Uint8Array(digest))
    .map((byte) => byte.toString(16).padStart(2, "0"))
    .join("");
}

async function setupDatabase() {
  let admin = new Client(ADMIN_DATABASE_URL);
  for (let attempt = 0;; attempt++) {
    try {
      await admin.connect();
      break;
    } catch (err) {
      if (attempt === 30) throw err;
      await new Promise((resolve) => setTimeout(resolve, 1000));
      admin = new Client(ADMIN_DATABASE_URL);
    }
  }
  await admin.queryArray(`DROP DATABASE IF EXISTS ${DATABASE} WITH (FORCE)`);
  await admin.queryArray(`CREATE DATABASE ${DATABASE}`);
  await admin.end();

  const url = new URL(ADMIN_DATABASE_URL);
  url.pathname = `/${DATABASE}`;
  const db = new Client(url.href);
  await db.connect();
  const migrations = [];
  for await (const entry of Deno.readDir(new URL("api/migrations/", root))) {
    if (entry.name.endsWith(".sql")) migrations.push(entry.name);
  }
  migrations.sort((a, b) => parseInt(a) - parseInt(b));
  for (const name of migrations) {
    const sql = await Deno.readTextFile(
      new URL(`api/migrations/${name}`, root),
    );
    try {
      await db.queryArray(sql);
    } catch (err) {
      throw new Error(`migration ${name} failed`, { cause: err });
    }
  }
  await db.queryArray(
    `INSERT INTO tokens (hash, user_id, type, description)
     VALUES ($1, $2, 'personal', 'worker smoke test')`,
    [await sha256Hex(TOKEN), SERVICE_ACCOUNT_ID],
  );
  await db.end();
}

async function startWorker(persistTo: string): Promise<Deno.ChildProcess> {
  const child = new Deno.Command("npx", {
    args: [
      "--yes",
      "wrangler@4",
      "dev",
      "--env=smoke",
      `--port=${PORT}`,
      `--persist-to=${persistTo}`,
    ],
    cwd: new URL("api/", root),
    stdin: "null",
  }).spawn();

  const deadline = Date.now() + 20 * 60 * 1000;
  while (Date.now() < deadline) {
    try {
      const resp = await fetch(`${API}/scopes/missing`);
      await resp.body?.cancel();
      if (resp.status === 404) return child;
    } catch {
      // not listening yet
    }
    await new Promise((resolve) => setTimeout(resolve, 1000));
  }
  await stopWorker(child);
  throw new Error("the worker did not start");
}

async function descendants(pid: number): Promise<number[]> {
  const { stdout } = await new Deno.Command("pgrep", {
    args: ["-P", String(pid)],
  }).output();
  const children = new TextDecoder().decode(stdout).split("\n")
    .filter((line) => line !== "").map(Number);
  const nested = await Promise.all(children.map(descendants));
  return [...nested.flat(), ...children];
}

// npx does not pass signals on to wrangler and workerd.
async function stopWorker(worker: Deno.ChildProcess) {
  for (const pid of [...await descendants(worker.pid), worker.pid]) {
    try {
      Deno.kill(pid, "SIGTERM");
    } catch {
      // already gone
    }
  }
  await worker.status;
}

async function api(
  method: string,
  path: string,
  init: { body?: BodyInit; headers?: Record<string, string> } = {},
): Promise<Response> {
  return await fetch(`${API}${path}`, {
    method,
    body: init.body,
    headers: { authorization: `Bearer ${TOKEN}`, ...init.headers },
  });
}

async function json(
  method: string,
  path: string,
  body?: unknown,
  // deno-lint-ignore no-explicit-any
): Promise<any> {
  const resp = await api(method, path, {
    body: body === undefined ? undefined : JSON.stringify(body),
    headers: { "content-type": "application/json" },
  });
  const text = await resp.text();
  assertEquals(resp.status, 200, `${method} ${path} (${text})`);
  return JSON.parse(text);
}

async function tarball(): Promise<Uint8Array<ArrayBuffer>> {
  const { success, stdout } = await new Deno.Command("tar", {
    args: ["-czf", "-", "-C", "api/testdata/tarballs/ok", "."],
    cwd: root,
    env: { COPYFILE_DISABLE: "1" },
    stderr: "inherit",
  }).output();
  if (!success) throw new Error("tar failed");
  return stdout;
}

async function run() {
  await json("POST", "/scopes", { scope: "scope", description: "smoke" });
  await json("POST", "/scopes/scope/packages", { package: "foo" });
  await json("POST", "/scopes/scope/packages", { package: "empty" });

  const deleted = await api("DELETE", "/scopes/scope/packages/empty");
  assertEquals(deleted.status, 204, "DELETE /scopes/scope/packages/empty");
  assertEquals((await deleted.bytes()).length, 0, "204 body length");

  const upload = await tarball();
  const published = await api(
    "POST",
    "/scopes/scope/packages/foo/versions/1.2.3?config=/jsr.json",
    { body: upload, headers: { "content-encoding": "gzip" } },
  );
  const publishedText = await published.text();
  assertEquals(published.status, 200, `publish (${publishedText})`);
  let task = JSON.parse(publishedText);
  for (let i = 0; i < 60 && task.status !== "success"; i++) {
    if (task.status === "failure") {
      throw new Error(`publish failed: ${JSON.stringify(task.error)}`);
    }
    await new Promise((resolve) => setTimeout(resolve, 1000));
    task = await json("GET", `/publishing_tasks/${task.id}`);
  }
  assertEquals(task.status, "success", "publishing task status");

  const version = await json(
    "GET",
    "/scopes/scope/packages/foo/versions/1.2.3",
  );
  assertEquals(version.version, "1.2.3", "published version");

  const download = await api(
    "GET",
    "/scopes/scope/packages/foo/versions/1.2.3/tarball",
  );
  assertEquals(download.status, 200, "tarball download");
  const downloaded = await download.bytes();
  assertEquals(downloaded.length, upload.length, "tarball length");
  assertEquals(
    downloaded.every((byte, i) => byte === upload[i]),
    true,
    "tarball bytes match the upload",
  );

  await json(
    "GET",
    "/scopes/scope/packages/foo/versions/1.2.3/dependencies/graph",
  );
  await json("GET", "/scopes/scope/packages/foo/versions/1.2.3/docs");

  // A request that leaks its runtime leaks three of the isolate's 4096 file
  // descriptors, which this many requests would use up.
  for (let i = 0; i < 1500; i += 20) {
    const responses = await Promise.all(
      Array.from({ length: 20 }, () => api("GET", "/scopes/scope")),
    );
    for (const resp of responses) {
      await resp.body?.cancel();
      assertEquals(resp.status, 200, `GET /scopes/scope (request ${i})`);
    }
  }
}

await setupDatabase();
const persistTo = await Deno.makeTempDir();
const worker = await startWorker(persistTo);
try {
  await run();
  console.log("worker smoke test passed");
} finally {
  await stopWorker(worker);
  await Deno.remove(persistTo, { recursive: true });
}
