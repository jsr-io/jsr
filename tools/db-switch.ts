#!/usr/bin/env -S deno run --allow-read --allow-write --allow-run --allow-env
// Copyright 2024 the JSR authors. All rights reserved. MIT license.

import { parseArgs } from "jsr:@std/cli@^1";
import { load } from "jsr:@std/dotenv@^0.225";
import { exists } from "jsr:@std/fs@^1";

const ENV_FILE = "api/.env";
const ENV_LOCAL_FILE = "api/.env.local";

interface DatabaseConfig {
  host: string;
  port: string;
  user: string;
  password: string;
  name: string;
}

async function getCurrentBranch(): Promise<string> {
  try {
    const cmd = new Deno.Command("git", {
      args: ["rev-parse", "--abbrev-ref", "HEAD"],
      stdout: "piped",
    });
    const { stdout } = await cmd.output();
    return new TextDecoder().decode(stdout).trim().replace(
      /[^a-zA-Z0-9_]/g,
      "_",
    );
  } catch {
    return "unknown";
  }
}

async function getMainDbName(): Promise<string> {
  const env = await load({ envPath: ENV_FILE });
  return env.MAIN_DB_NAME || "registry";
}

async function parseCurrentConfig(): Promise<DatabaseConfig> {
  const env = await load({ envPath: ENV_FILE });
  const databaseUrl = env.DATABASE_URL || "";

  try {
    const url = new URL(databaseUrl);
    if (url.protocol !== "postgres:") throw new Error("Invalid protocol");

    return {
      user: decodeURIComponent(url.username),
      password: decodeURIComponent(url.password),
      host: url.hostname,
      port: url.port || "5432",
      name: url.pathname.slice(1),
    };
  } catch {
    const mainDb = await getMainDbName();
    return {
      host: "localhost",
      port: "5432",
      user: "user",
      password: "password",
      name: mainDb,
    };
  }
}

function buildDatabaseUrl(config: DatabaseConfig, dbName: string): string {
  return `postgres://${config.user}:${config.password}@${config.host}:${config.port}/${dbName}`;
}

async function dbExists(
  config: DatabaseConfig,
  dbName: string,
): Promise<boolean> {
  try {
    const cmd = new Deno.Command("psql", {
      args: ["-h", config.host, "-p", config.port, "-U", config.user, "-lqt"],
      env: { PGPASSWORD: config.password },
      stdout: "piped",
    });
    const { stdout } = await cmd.output();
    return new TextDecoder().decode(stdout).includes(dbName);
  } catch {
    return false;
  }
}

async function createDatabase(
  config: DatabaseConfig,
  sourceDb: string,
  targetDb: string,
): Promise<void> {
  console.log(`Creating database copy from ${sourceDb} to ${targetDb}...`);

  const cmd = new Deno.Command("psql", {
    args: [
      "-h",
      config.host,
      "-p",
      config.port,
      "-U",
      config.user,
      "-c",
      `CREATE DATABASE "${targetDb}" WITH TEMPLATE "${sourceDb}";`,
    ],
    env: { PGPASSWORD: config.password },
  });

  const output = await cmd.output();
  if (!output.success) {
    const errorMessage = new TextDecoder().decode(output.stderr);
    throw new Error(
      `Failed to create database: ${errorMessage || "Unknown error"}`,
    );
  }

  console.log(`Database created successfully!`);
}

async function copyFromMain(
  method: "env-local" | "export" = "env-local",
): Promise<void> {
  const branch = await getCurrentBranch();
  const config = await parseCurrentConfig();
  const mainDb = await getMainDbName();
  const branchDb = `${mainDb}_${branch}`;

  console.log(`Switching to branch database: ${branchDb}`);

  if (!await dbExists(config, branchDb)) {
    console.log(`Branch database does not exist. Creating...`);
    await createDatabase(config, mainDb, branchDb);
  }

  const databaseUrl = buildDatabaseUrl(config, branchDb);

  switch (method) {
    case "env-local":
      await Deno.writeTextFile(
        ENV_LOCAL_FILE,
        `DATABASE_URL=${databaseUrl}\n`,
      );
      console.log(
        `Created ${ENV_LOCAL_FILE} - restart your app to use branch database`,
      );
      break;
    case "export":
      console.log(`\nexport DATABASE_URL="${databaseUrl}"`);
      break;
  }
}

async function switchToMain(): Promise<void> {
  const mainDb = await getMainDbName();
  console.log(`Switching to main database: ${mainDb}`);

  if (await exists(ENV_LOCAL_FILE)) {
    await Deno.remove(ENV_LOCAL_FILE);
    console.log(`Removed ${ENV_LOCAL_FILE}`);
  }
}

async function showCurrent(): Promise<void> {
  const config = await parseCurrentConfig();
  const mainDb = await getMainDbName();
  const hasEnvLocal = await exists(ENV_LOCAL_FILE);

  console.log(`Current database: ${config.name}`);

  if (hasEnvLocal) {
    const envLocal = await load({ envPath: ENV_LOCAL_FILE });
    console.log(`Override active via ${ENV_LOCAL_FILE}: ${envLocal.DATABASE_URL}`);
  } else if (config.name === mainDb) {
    console.log(`Using main database`);
  } else {
    console.log(`Using branch database`);
  }
}

async function listDatabases(): Promise<void> {
  const config = await parseCurrentConfig();
  const mainDb = await getMainDbName();
  const currentDb = config.name;

  const cmd = new Deno.Command("psql", {
    args: [
      "-h",
      config.host,
      "-p",
      config.port,
      "-U",
      config.user,
      "-t",
      "-c",
      `SELECT datname FROM pg_database WHERE datname = '${mainDb}' OR datname LIKE '${mainDb}_%' ORDER BY datname;`,
    ],
    env: { PGPASSWORD: config.password },
    stdout: "piped",
  });

  const output = await cmd.output();
  if (!output.success) {
    throw new Error("Failed to list databases");
  }

  const databases = new TextDecoder().decode(output.stdout)
    .split("\n")
    .map((db) => db.trim())
    .filter((db) => db.length > 0);

  console.log("Branch databases:");
  for (const db of databases) {
    const marker = db === currentDb ? " <- (current)" : "";
    const icon = db === mainDb ? "[main]" : "[branch]";
    console.log(`${icon} ${db}${marker}`);
  }
}

async function createEmpty(
  method: "env-local" | "export" = "env-local",
): Promise<void> {
  const branch = await getCurrentBranch();
  const config = await parseCurrentConfig();
  const mainDb = await getMainDbName();
  const blankDb = `${mainDb}_${branch}`;

  console.log(`Creating blank database: ${blankDb}`);

  if (await dbExists(config, blankDb)) {
    console.log(`Database ${blankDb} already exists`);
  } else {
    const cmd = new Deno.Command("psql", {
      args: [
        "-h",
        config.host,
        "-p",
        config.port,
        "-U",
        config.user,
        "-c",
        `CREATE DATABASE "${blankDb}";`,
      ],
      env: { PGPASSWORD: config.password },
    });

    const output = await cmd.output();
    if (!output.success) {
      const errorMessage = new TextDecoder().decode(output.stderr);
      throw new Error(
        `Failed to create blank database: ${errorMessage || "Unknown error"}`,
      );
    }

    console.log(`Blank database created successfully!`);
  }

  const databaseUrl = buildDatabaseUrl(config, blankDb);

  switch (method) {
    case "env-local":
      await Deno.writeTextFile(
        ENV_LOCAL_FILE,
        `DATABASE_URL=${databaseUrl}\n`,
      );
      console.log(
        `Created ${ENV_LOCAL_FILE} - restart your app to use blank database`,
      );
      break;
    case "export":
      console.log(`\nexport DATABASE_URL="${databaseUrl}"`);
      break;
  }
}

async function cleanDatabases(): Promise<void> {
  const config = await parseCurrentConfig();
  const mainDb = await getMainDbName();

  const cmd = new Deno.Command("psql", {
    args: [
      "-h",
      config.host,
      "-p",
      config.port,
      "-U",
      config.user,
      "-t",
      "-c",
      `SELECT datname FROM pg_database WHERE datname LIKE '${mainDb}_%' ORDER BY datname;`,
    ],
    env: { PGPASSWORD: config.password },
    stdout: "piped",
  });

  const output = await cmd.output();
  if (!output.success) {
    throw new Error("Failed to list databases");
  }

  const databases = new TextDecoder().decode(output.stdout)
    .split("\n")
    .map((db) => db.trim())
    .filter((db) => db.length > 0);

  if (databases.length === 0) {
    console.log("No branch databases to clean up.");
    return;
  }

  console.log(`Dropping ${databases.length} branch database(s)...`);
  for (const db of databases) {
    const dropCmd = new Deno.Command("psql", {
      args: [
        "-h",
        config.host,
        "-p",
        config.port,
        "-U",
        config.user,
        "-c",
        `DROP DATABASE "${db}";`,
      ],
      env: { PGPASSWORD: config.password },
    });

    const dropOutput = await dropCmd.output();
    if (!dropOutput.success) {
      const errorMessage = new TextDecoder().decode(dropOutput.stderr);
      console.error(`Failed to drop ${db}: ${errorMessage}`);
    } else {
      console.log(`Dropped ${db}`);
    }
  }

  if (await exists(ENV_LOCAL_FILE)) {
    await Deno.remove(ENV_LOCAL_FILE);
    console.log(`Removed ${ENV_LOCAL_FILE}`);
  }
}

function showHelp(): void {
  console.log(`Database Switch Helper

Usage: deno task db:switch [command] [options]

Commands:
  switch    - Switch to a branch database copied from main (creates if needed)
  empty     - Switch to an empty branch database (creates if needed)
  main      - Switch back to main database
  current   - Show current database
  list      - List branch databases
  clean     - Drop all branch databases

Options:
  --method=env-local   - Use api/.env.local (default)
  --method=export      - Print export command`);
}

if (import.meta.main) {
  const args = parseArgs(Deno.args);
  const command = args._[0] as string || "help";
  const method = args.method as "env-local" | "export" || "env-local";

  try {
    switch (command) {
      case "switch":
        await copyFromMain(method);
        break;
      case "empty":
        await createEmpty(method);
        break;
      case "main":
        await switchToMain();
        break;
      case "current":
        await showCurrent();
        break;
      case "list":
        await listDatabases();
        break;
      case "clean":
        await cleanDatabases();
        break;
      default:
        showHelp();
    }
  } catch (error) {
    console.error(
      `Error: ${error instanceof Error ? error.message : String(error)}`,
    );
    Deno.exit(1);
  }
}
