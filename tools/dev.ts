#!/usr/bin/env -S deno run -A
// Copyright 2024 the JSR authors. All rights reserved. MIT license.

import {
  blue,
  bold,
  brightBlue,
  cyan,
  dim,
  green,
  magenta,
  red,
  yellow,
} from "@std/fmt/colors";
import { TextLineStream } from "@std/streams";

const CLEAR_LINE = "\x1b[2K";

function getBinDir(): string {
  const os = Deno.build.os;
  const arch = Deno.build.arch;
  if (os === "darwin" && arch === "aarch64") return "darwin-arm64";
  if (os === "darwin" && arch === "x86_64") return "darwin-amd64";
  if (os === "linux" && arch === "x86_64") return "linux-amd64";
  throw new Error(`Unsupported platform: ${os}-${arch}`);
}

interface ProcessDef {
  name: string;
  cmd: string | URL;
  args?: string[];
  cwd?: string;
  env?: Record<string, string>;
  color: (s: string) => string;
}

function getProcessDefs(noDockerPostgres: boolean): ProcessDef[] {
  const binDir = new URL(import.meta.resolve(`./bin/${getBinDir()}`)).pathname;
  const rootDir = new URL(import.meta.resolve(`../`)).pathname.slice(0, -1);

  const defs: ProcessDef[] = [];

  if (Deno.build.os === "linux") {
    if (!noDockerPostgres) {
      defs.push({
        name: "postgres",
        cmd: "docker",
        args: ["compose", "up", "postgres"],
        color: yellow,
      });
    }
    defs.push({
      name: "jaeger",
      cmd: "docker",
      args: ["compose", "up", "jaeger"],
      color: cyan,
    });
  } else {
    defs.push({
      name: "jaeger",
      cmd: `${binDir}/jaeger-all-in-one`,
      color: cyan,
    });
  }

  defs.push({
    name: "gcs",
    cmd: `${binDir}/fake-gcs-server`,
    args: [
      "-scheme",
      "http",
      "-port",
      "4080",
      "-filesystem-root=.gcs",
      "-public-host=localhost:4080",
    ],
    cwd: rootDir,
    color: blue,
  });

  defs.push({
    name: "minio",
    cmd: `${binDir}/minio`,
    args: ["server", "--quiet", ".minio"],
    cwd: rootDir,
    color: brightBlue,
  });

  defs.push({
    name: "lb",
    cmd: Deno.execPath(),
    args: ["run", "-A", "--watch", "local.ts"],
    cwd: `${rootDir}/lb`,
    color: red,
  });

  defs.push({
    name: "api",
    cmd: "cargo",
    args: ["run"],
    cwd: `${rootDir}/api`,
    color: magenta,
  });

  defs.push({
    name: "frontend",
    cmd: Deno.execPath(),
    args: ["task", "dev"],
    cwd: `${rootDir}/frontend`,
    env: { OLTP_ENDPOINT: "http://localhost:4318" },
    color: green,
  });

  return defs;
}

interface ManagedProcess {
  def: ProcessDef;
  child: Deno.ChildProcess;
  abortController: AbortController;
}

class DevRunner {
  #processes = new Map<string, ManagedProcess>();
  #maxNameLen = 0;
  #noDockerPostgres: boolean;
  #inputBuf = "";
  #encoder = new TextEncoder();
  #shuttingDown = false;

  constructor(noDockerPostgres: boolean) {
    this.#noDockerPostgres = noDockerPostgres;
  }

  async run() {
    const defs = getProcessDefs(this.#noDockerPostgres);
    this.#maxNameLen = Math.max(...defs.map((d) => d.name.length));

    for (let i = 0; i < defs.length; i++) {
      this.#spawn(defs[i]);
    }

    this.#renderPrompt();
    await this.#readInput();
  }

  #spawn(def: ProcessDef) {
    const abortController = new AbortController();
    const child = new Deno.Command(def.cmd, {
      args: def.args,
      cwd: def.cwd,
      env: def.env ? { ...Deno.env.toObject(), ...def.env } : undefined,
      stdout: "piped",
      stderr: "piped",
      signal: abortController.signal,
    }).spawn();

    const proc: ManagedProcess = { def, child, abortController };
    this.#processes.set(def.name, proc);

    this.#pipeStream(proc, child.stdout, false);
    this.#pipeStream(proc, child.stderr, true);

    child.status.then((status) => {
      if (!this.#shuttingDown) {
        this.#log(
          proc,
          `process exited with code ${status.code}`,
          true,
        );
      }
    });
  }

  async #pipeStream(
    proc: ManagedProcess,
    stream: ReadableStream<Uint8Array>,
    isStderr: boolean,
  ) {
    for await (
      const line of stream.pipeThrough(new TextDecoderStream()).pipeThrough(
        new TextLineStream(),
      )
    ) {
      this.#log(proc, line, isStderr);
    }
  }

  #log(proc: ManagedProcess, message: string, isStderr: boolean) {
    const name = proc.def.name.padEnd(this.#maxNameLen);
    // Clear prompt line, write log, re-render prompt
    this.#write(
      `\r${CLEAR_LINE}${proc.def.color(bold(name))} ${dim("|")} ${
        isStderr ? dim(message) : message
      }\n`,
    );
    this.#renderPrompt();
  }

  #renderPrompt() {
    this.#write(
      `\r${CLEAR_LINE}${bold("> ")}${this.#inputBuf}`,
    );
  }

  #write(data: string) {
    Deno.stdout.writeSync(this.#encoder.encode(data));
  }

  async #readInput() {
    Deno.stdin.setRaw(true);
    const buf = new Uint8Array(256);
    try {
      while (true) {
        const n = await Deno.stdin.read(buf);
        if (n === null) break;

        for (let i = 0; i < n; i++) {
          const byte = buf[i];

          // Ctrl+C
          if (byte === 3) {
            await this.#shutdown();
            return;
          }

          // Enter
          if (byte === 13 || byte === 10) {
            const input = this.#inputBuf.trim();
            this.#inputBuf = "";
            this.#write(`\r${CLEAR_LINE}`);
            if (input.length > 0) {
              await this.#handleCommand(input);
            }
            this.#renderPrompt();
            continue;
          }

          // Backspace / Delete
          if (byte === 127 || byte === 8) {
            if (this.#inputBuf.length > 0) {
              this.#inputBuf = this.#inputBuf.slice(0, -1);
              this.#renderPrompt();
            }
            continue;
          }

          // Ctrl+W - delete word
          if (byte === 23) {
            this.#inputBuf = this.#inputBuf.replace(/\s*\S+\s*$/, "");
            this.#renderPrompt();
            continue;
          }

          // Skip escape sequences
          if (byte === 27) {
            // consume remaining escape bytes
            while (i + 1 < n && buf[i + 1] >= 0x20 && buf[i + 1] <= 0x7e) {
              i++;
              if (buf[i] >= 0x40) break;
            }
            continue;
          }

          // Regular printable character
          if (byte >= 32 && byte < 127) {
            this.#inputBuf += String.fromCharCode(byte);
            this.#renderPrompt();
          }
        }
      }
    } finally {
      Deno.stdin.setRaw(false);
    }
  }

  async #handleCommand(input: string) {
    const parts = input.split(/\s+/);
    const cmd = parts[0].toLowerCase();
    const arg = parts[1]?.toLowerCase();

    switch (cmd) {
      case "restart":
      case "r": {
        if (!arg) {
          this.#writeSystem("Usage: restart <name> or restart all");
          this.#writeSystem(
            `Available: ${[...this.#processes.keys()].join(", ")}, all`,
          );
          return;
        }
        if (arg === "all") {
          for (const name of this.#processes.keys()) {
            await this.#restartProcess(name);
          }
        } else if (this.#processes.has(arg)) {
          await this.#restartProcess(arg);
        } else {
          this.#writeSystem(
            `Unknown process "${arg}". Available: ${
              [...this.#processes.keys()].join(", ")
            }`,
          );
        }
        return;
      }
      case "help": {
        this.#writeSystem("Commands:");
        this.#writeSystem("  restart <name|all>  Restart a process or all");
        this.#writeSystem("  quit                Shutdown all and exit");
        this.#writeSystem("  help                Show this help");
        return;
      }
      case "stop":
      case "quit":
      case "exit":
      case "q": {
        await this.#shutdown();
        return;
      }
      default: {
        this.#writeSystem(
          `Unknown command "${cmd}". Type "help" for available commands.`,
        );
      }
    }
  }

  async #restartProcess(name: string) {
    this.#writeSystem(`Restarting "${name}"...`);
    const proc = this.#processes.get(name)!;
    const def = proc.def;
    await this.#stopProcess(name);
    this.#spawn(def);
    this.#writeSystem(`"${name}" restarted.`);
  }

  async #stopProcess(name: string) {
    const proc = this.#processes.get(name);
    if (!proc) return;
    try {
      proc.abortController.abort();
      await proc.child.status;
    } catch {
      // already exited
    }
    this.#processes.delete(name);
  }

  #writeSystem(msg: string) {
    this.#write(
      `\r${CLEAR_LINE}${bold("\x1b[90m[dev]")} ${msg}\n`,
    );
  }

  async #shutdown() {
    this.#shuttingDown = true;
    this.#writeSystem("Shutting down all processes...");
    const stops = [...this.#processes.keys()].map((name) =>
      this.#stopProcess(name)
    );
    await Promise.all(stops);
    this.#writeSystem("All processes stopped. Goodbye.");
    try {
      Deno.stdin.setRaw(false);
    } catch {
      // may already be reset
    }
    Deno.exit(0);
  }
}

// --- Setup command ---

const HOSTS_ENTRIES = [
  "127.0.0.1       jsr.test",
  "127.0.0.1       api.jsr.test",
  "127.0.0.1       npm.jsr.test",
];

async function isWSL(): Promise<boolean> {
  if (Deno.build.os !== "linux") return false;
  try {
    const release = await Deno.readTextFile("/proc/version");
    return /microsoft|wsl/i.test(release);
  } catch {
    return false;
  }
}

function getHostsPaths(wsl: boolean): string[] {
  const paths = ["/etc/hosts"];
  if (wsl) {
    paths.push("/mnt/c/Windows/System32/drivers/etc/hosts");
  }
  return paths;
}

async function ensureHostsEntries(
  hostsPath: string,
): Promise<"ok" | "warn" | "fail"> {
  let content: string;
  try {
    content = await Deno.readTextFile(hostsPath);
  } catch {
    console.log(`    cannot read file`);
    console.log("    add the following entries manually:");
    for (const entry of HOSTS_ENTRIES) {
      console.log(`      ${entry}`);
    }
    return "warn";
  }

  const missing = HOSTS_ENTRIES.filter((entry) => {
    const hostname = entry.split(/\s+/)[1];
    return !content.split("\n").some((line) => {
      const trimmed = line.trim();
      if (trimmed.startsWith("#")) return false;
      return trimmed.split(/\s+/).includes(hostname);
    });
  });

  if (missing.length === 0) {
    console.log("    all entries present");
    return "ok";
  }

  console.log(`    missing ${missing.length} host(s), adding...`);
  const toAppend = "\n# JSR local development\n" +
    missing.join("\n") + "\n";

  const cmd = new Deno.Command("sudo", {
    args: ["tee", "-a", hostsPath],
    stdin: "piped",
    stdout: "null",
    stderr: "piped",
  });
  const child = cmd.spawn();
  const writer = child.stdin.getWriter();
  await writer.write(new TextEncoder().encode(toAppend));
  await writer.close();
  const status = await child.status;
  if (status.success) {
    console.log("    entries added");
    return "ok";
  }

  console.log("    failed to write. Add manually:");
  for (const entry of missing) {
    console.log(`      ${entry}`);
  }
  return "warn";
}

async function checkCommand(
  cmd: string,
  args: string[],
): Promise<string | null> {
  try {
    const proc = new Deno.Command(cmd, {
      args,
      stdout: "piped",
      stderr: "piped",
    });
    const out = await proc.output();
    if (out.success) {
      return new TextDecoder().decode(out.stdout).trim();
    }
    return null;
  } catch {
    return null;
  }
}

async function runSetupFrontend() {
  console.log(`${bold("JSR Frontend Setup")}\n`);

  const wsl = await isWSL();
  const hostsPaths = getHostsPaths(wsl);

  console.log(`${bold("Check hosts entries")}${wsl ? " (WSL + Windows)" : ""}`);
  let result: "ok" | "warn" | "fail" = "ok";
  for (const hostsPath of hostsPaths) {
    console.log(`  ${hostsPath}:`);
    const pathResult = await ensureHostsEntries(hostsPath);
    if (pathResult === "fail") result = "fail";
    else if (pathResult === "warn" && result !== "fail") result = "warn";
  }

  console.log();
  if (result === "fail") {
    console.log(
      `${bold("\x1b[31mSetup failed.")} Fix the errors above and re-run.`,
    );
    Deno.exit(1);
  } else if (result === "warn") {
    console.log(
      `${
        bold("\x1b[33mSetup finished with warnings.")
      } Review the items above.`,
    );
  } else {
    console.log(
      `${
        bold("\x1b[32mSetup complete!")
      } Run 'deno task prod:frontend' to start.`,
    );
  }
}

async function runSetup(noDockerPostgres: boolean) {
  const rootDir = new URL(import.meta.resolve(`../`)).pathname.slice(0, -1);
  console.log(`${bold("JSR Development Setup")}\n`);

  const isMacOS = Deno.build.os === "darwin";

  interface Step {
    name: string;
    fn: () => Promise<"ok" | "warn" | "fail">;
  }

  const steps: Step[] = [];

  // 1. Check Deno
  steps.push({
    name: "Check Deno",
    fn: async () => {
      const ver = await checkCommand("deno", ["--version"]);
      if (ver) {
        console.log(`  ${ver.split("\n")[0]}`);
        return "ok";
      }
      console.log("  deno not found. Install from https://deno.com");
      return "fail";
    },
  });

  // 2. Check Rust/Cargo
  steps.push({
    name: "Check Rust/Cargo",
    fn: async () => {
      const ver = await checkCommand("cargo", ["--version"]);
      if (ver) {
        console.log(`  ${ver}`);
        return "ok";
      }
      console.log("  cargo not found. Install from https://rustup.rs");
      return "fail";
    },
  });

  // 3. Check Docker (Linux only)
  steps.push({
    name: "Check Docker",
    fn: async () => {
      if (Deno.build.os !== "linux") {
        console.log("  skipped (not needed on macOS)");
        return "ok";
      }
      const ver = await checkCommand("docker", ["--version"]);
      if (ver) {
        console.log(`  ${ver}`);
        return "ok";
      }
      console.log("  docker not found. Install from https://docker.com");
      return "fail";
    },
  });

  // 4. Check Postgres (macOS only - on Linux it runs via docker)
  if (isMacOS) {
    steps.push({
      name: "Check PostgreSQL",
      fn: async () => {
        const ver = await checkCommand("psql", ["--version"]);
        if (ver) {
          console.log(`  ${ver}`);
          return "ok";
        }
        console.log(
          "  psql not found. Install PostgreSQL: brew install postgresql",
        );
        return "fail";
      },
    });
  }

  // 5. Check sqlx-cli
  steps.push({
    name: "Check sqlx-cli",
    fn: async () => {
      const ver = await checkCommand("sqlx", ["--version"]);
      if (ver) {
        console.log(`  ${ver}`);
        return "ok";
      }
      console.log(
        "  sqlx not found. Install with: cargo install sqlx-cli",
      );
      return "fail";
    },
  });

  // 6. /etc/hosts entries
  const wsl = await isWSL();
  steps.push({
    name: `Check hosts entries${wsl ? " (WSL + Windows)" : ""}`,
    fn: async () => {
      const hostsPaths = getHostsPaths(wsl);
      let result: "ok" | "warn" | "fail" = "ok";

      for (const hostsPath of hostsPaths) {
        console.log(`  ${hostsPath}:`);
        const pathResult = await ensureHostsEntries(hostsPath);
        if (pathResult === "fail") result = "fail";
        else if (pathResult === "warn" && result !== "fail") result = "warn";
      }

      return result;
    },
  });

  // 7. Install frontend dependencies
  steps.push({
    name: "Install frontend dependencies",
    fn: async () => {
      const cmd = new Deno.Command("deno", {
        args: ["install"],
        cwd: `${rootDir}/frontend`,
        stdout: "piped",
        stderr: "piped",
      });
      const out = await cmd.output();
      if (out.success) {
        console.log("  dependencies installed");
        return "ok";
      }
      const err = new TextDecoder().decode(out.stderr);
      console.log(`  failed: ${err}`);
      return "fail";
    },
  });

  // 8. Check api/.env
  steps.push({
    name: "Check api/.env",
    fn: async () => {
      try {
        await Deno.stat(`${rootDir}/api/.env`);
        console.log("  api/.env exists");
        return "ok";
      } catch {
        console.log("  api/.env missing. Creating from .env.example...");
        await Deno.copyFile(
          `${rootDir}/api/.env.example`,
          `${rootDir}/api/.env`,
        );
        console.log("  copied api/.env.example -> api/.env");
        console.log(`  ${red(bold("ACTION REQUIRED:"))} Edit api/.env to set:`);
        console.log("    - DATABASE_URL (your local Postgres connection)");
        console.log("    - GITHUB_CLIENT_ID and GITHUB_CLIENT_SECRET");
        console.log(
          "      (create a GitHub App at https://github.com/settings/apps/new)",
        );
        console.log("      Callback URL: http://jsr.test/login/callback");
        return "warn";
      }
    },
  });

  // 9. Create Postgres database
  steps.push({
    name: "Create PostgreSQL database",
    fn: async () => {
      if (Deno.build.os === "linux" && !noDockerPostgres) {
        console.log("  skipped (PostgreSQL runs via docker compose)");
        console.log(
          "  after starting services, run: createdb -h localhost -U user registry",
        );
        return "warn";
      }

      // Check if database exists
      const check = await checkCommand("psql", [
        "-lqt",
      ]);
      if (check && check.includes("registry")) {
        console.log("  database 'registry' already exists");
        return "ok";
      }
      // Try to create
      const cmd = new Deno.Command("createdb", {
        args: ["registry"],
        stdout: "piped",
        stderr: "piped",
      });
      const out = await cmd.output();
      if (out.success) {
        console.log("  database 'registry' created");
        return "ok";
      }
      const err = new TextDecoder().decode(out.stderr).trim();
      if (err.includes("already exists")) {
        console.log("  database 'registry' already exists");
        return "ok";
      }
      console.log(`  failed to create database: ${err}`);
      console.log("  run manually: createdb registry");
      return "warn";
    },
  });

  // 10. Run database migrations
  steps.push({
    name: "Run database migrations",
    fn: async () => {
      const sqlxVer = await checkCommand("sqlx", ["--version"]);
      if (!sqlxVer) {
        console.log("  sqlx not installed, skipping migrations");
        console.log("  install with: cargo install sqlx-cli");
        console.log("  then run: deno task db:migrate");
        return "warn";
      }

      // Check if DATABASE_URL is available
      let hasDbUrl = false;
      try {
        const envContent = await Deno.readTextFile(`${rootDir}/api/.env`);
        hasDbUrl = envContent.includes("DATABASE_URL=") &&
          !envContent.includes("DATABASE_URL=postgres://<");
      } catch {
        // no .env file
      }

      if (!hasDbUrl) {
        console.log(
          "  DATABASE_URL not configured in api/.env, skipping migrations",
        );
        console.log("  after configuring, run: cd api && sqlx migrate run");
        return "warn";
      }

      const cmd = new Deno.Command(Deno.execPath(), {
        args: ["task", "db:migrate"],
        cwd: rootDir,
        stdout: "piped",
        stderr: "piped",
      });
      const out = await cmd.output();
      if (out.success) {
        console.log("  migrations applied");
        return "ok";
      }
      const err = new TextDecoder().decode(out.stderr).trim();
      console.log(`  migration failed: ${err}`);
      if (err.includes("role") && err.includes("does not exist")) {
        console.log("  hint: run 'createuser -s postgres' and try again");
      }
      return "warn";
    },
  });

  // Run all steps
  let hasFailure = false;
  let hasWarning = false;
  for (const step of steps) {
    console.log(`\n${bold(step.name)}`);
    const result = await step.fn();
    if (result === "fail") {
      console.log(`  \x1b[31m${bold("FAILED")}`);
      hasFailure = true;
    } else if (result === "warn") {
      console.log(`  \x1b[33m${bold("WARNING")}`);
      hasWarning = true;
    }
  }

  console.log();
  if (hasFailure) {
    console.log(
      `${bold("\x1b[31mSetup failed.")} Fix the errors above and re-run setup.`,
    );
    Deno.exit(1);
  } else if (hasWarning) {
    console.log(
      `${
        bold("\x1b[33mSetup finished with warnings.")
      } Review the items above, then run 'deno task dev' to start.`,
    );
  } else {
    console.log(
      `${
        bold("\x1b[32mSetup complete!")
      } Run 'deno task dev' to start development.`,
    );
  }
}

// --- Main ---

const noDockerPostgres = Deno.args.includes("--no-docker-postgres");

const subcommand = Deno.args.find((a) => !a.startsWith("-"));

const setupArg = Deno.args.find((a) => !a.startsWith("-") && a !== "setup");

if (subcommand === "setup" && setupArg === "frontend") {
  await runSetupFrontend();
} else if (subcommand === "setup") {
  await runSetup(noDockerPostgres);
} else if (subcommand === "start") {
  const startIdx = Deno.args.indexOf("start");
  const rest = Deno.args.slice(startIdx + 1).filter((a) => !a.startsWith("-"));
  const name = rest[0];
  const extraArgs = rest.slice(1);
  const defs = getProcessDefs(noDockerPostgres);
  if (!name) {
    console.log(`Usage: start <name> [args...]`);
    console.log(`Available: ${defs.map((d) => d.name).join(", ")}`);
    Deno.exit(1);
  }
  const def = defs.find((d) => d.name === name);
  if (!def) {
    console.log(`Unknown process "${name}".`);
    console.log(`Available: ${defs.map((d) => d.name).join(", ")}`);
    Deno.exit(1);
  }
  console.log(def.cmd);
  const child = new Deno.Command(def.cmd, {
    args: [...(def.args ?? []), ...extraArgs],
    cwd: def.cwd,
    env: def.env ? { ...Deno.env.toObject(), ...def.env } : undefined,
    stdin: "inherit",
    stdout: "inherit",
    stderr: "inherit",
  }).spawn();
  const status = await child.status;
  Deno.exit(status.code);
} else {
  const runner = new DevRunner(noDockerPostgres);
  await runner.run();
}
