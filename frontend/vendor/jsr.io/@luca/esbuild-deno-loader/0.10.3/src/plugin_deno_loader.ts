import type * as esbuild from "./esbuild_types.ts";
import { dirname, join } from "jsr:@std/path@0.213";
import { NativeLoader } from "./loader_native.ts";
import { PortableLoader } from "./loader_portable.ts";
import { isInNodeModules } from "./shared.ts";
import {
  esbuildResolutionToURL,
  isNodeModulesResolution,
  Loader,
  readDenoConfig,
  urlToEsbuildResolution,
} from "./shared.ts";

/** Options for the {@link denoLoaderPlugin}. */
export interface DenoLoaderPluginOptions {
  /**
   * Specify which loader to use. By default this will use the `native` loader,
   * unless the `--allow-run` permission has not been given.
   *
   * See {@link denoLoaderPlugin} for more information on the different loaders.
   */
  loader?: "native" | "portable";

  /**
   * Specify the path to a deno.json config file to use. This is equivalent to
   * the `--config` flag to the Deno executable. This path must be absolute.
   *
   * NOTE: Import maps in the config file are not used to inform resolution, as
   * this has already been done by the `denoResolverPlugin`. This option is only
   * used when specifying `loader: "native"` to more efficiently load modules
   * from the cache. When specifying `loader: "native"`, this option must be in
   * sync with the `configPath` option for `denoResolverPlugin`.
   */
  configPath?: string;
  /**
   * Specify a URL to an import map file to use when resolving import
   * specifiers. This is equivalent to the `--import-map` flag to the Deno
   * executable. This URL may be remote or a local file URL.
   *
   * If this option is not specified, the deno.json config file is consulted to
   * determine what import map to use, if any.
   *
   * NOTE: Import maps in the config file are not used to inform resolution, as
   * this has already been done by the `denoResolverPlugin`. This option is only
   * used when specifying `loader: "native"` to more efficiently load modules
   * from the cache. When specifying `loader: "native"`, this option must be in
   * sync with the `importMapURL` option for `denoResolverPlugin`.
   */
  importMapURL?: string;
  /**
   * Specify the path to a lock file to use. This is equivalent to the `--lock`
   * flag to the Deno executable. This path must be absolute.
   *
   * If this option is not specified, the deno.json config file is consulted to
   * determine what import map to use, if any.
   *
   * A lockfile must be present to resolve `jsr:` specifiers with the `portable`
   * loader. When using the `native` loader, a lockfile is not required, but to
   * ensure dependencies are de-duplicated correctly, it is recommended to use a
   * lockfile.
   *
   * NOTE: when using `loader: "portable"`, integrity checks are not performed
   * for ESM modules.
   */
  lockPath?: string;
  /**
   * Specify whether to generate and use a local `node_modules` directory when
   * using the `native` loader. This is equivalent to the `--node-modules-dir`
   * flag to the Deno executable.
   *
   * This option is ignored when using the `portable` loader, as the portable
   * loader always uses a local `node_modules` directory.
   */
  nodeModulesDir?: boolean;
}

const LOADERS = ["native", "portable"] as const;

/** The default loader to use. */
export const DEFAULT_LOADER: "native" | "portable" =
  await Deno.permissions.query({ name: "run" })
      .then((res) => res.state !== "granted")
    ? "portable"
    : "native";

const BUILTIN_NODE_MODULES = new Set([
  "assert",
  "assert/strict",
  "async_hooks",
  "buffer",
  "child_process",
  "cluster",
  "console",
  "constants",
  "crypto",
  "dgram",
  "diagnostics_channel",
  "dns",
  "dns/promises",
  "domain",
  "events",
  "fs",
  "fs/promises",
  "http",
  "http2",
  "https",
  "module",
  "net",
  "os",
  "path",
  "path/posix",
  "path/win32",
  "perf_hooks",
  "process",
  "punycode",
  "querystring",
  "repl",
  "readline",
  "stream",
  "stream/consumers",
  "stream/promises",
  "stream/web",
  "string_decoder",
  "sys",
  "test",
  "timers",
  "timers/promises",
  "tls",
  "tty",
  "url",
  "util",
  "util/types",
  "v8",
  "vm",
  "worker_threads",
  "zlib",
]);

/**
 * The Deno loader plugin for esbuild. This plugin will load fully qualified
 * `file`, `http`, `https`, and `data` URLs.
 *
 * **Note** that this plugin does not do relative->absolute specifier
 * resolution, or import map resolution. You must use the `denoResolverPlugin`
 * _before_ the `denoLoaderPlugin` to do that.
 *
 * This plugin can be backed by two different loaders, the `native` loader and
 * the `portable` loader.
 *
 * ### Native Loader
 *
 * The native loader shells out to the Deno executable under the hood to load
 * files. Requires `--allow-read` and `--allow-run`. In this mode the download
 * cache is shared with the Deno executable. This mode respects deno.lock,
 * DENO_DIR, DENO_AUTH_TOKENS, and all similar loading configuration. Files are
 * cached on disk in the same Deno cache as the Deno executable, and will not be
 * re-downloaded on subsequent builds.
 *
 * NPM specifiers can be used in the native loader without requiring a local
 * `node_modules` directory. NPM packages are resolved, downloaded, cached, and
 * loaded in the same way as the Deno executable does.
 *
 * JSR specifiers can be used without restrictions in the native loader. To
 * ensure dependencies are de-duplicated correctly, it is recommended to use a
 * lockfile.
 *
 * ### Portable Loader
 *
 * The portable loader does module downloading and caching with only Web APIs.
 * Requires `--allow-read` and/or `--allow-net`. This mode does not respect
 * deno.lock, DENO_DIR, DENO_AUTH_TOKENS, or any other loading configuration. It
 * does not cache downloaded files. It will re-download files on every build.
 *
 * NPM specifiers can be used in the portable loader, but require a local
 * `node_modules` directory. The `node_modules` directory must be created prior
 * using Deno's `--node-modules-dir` flag.
 *
 * JSR specifiers require a lockfile to be present to resolve.
 */
export function denoLoaderPlugin(
  options: DenoLoaderPluginOptions = {},
): esbuild.Plugin {
  const loader = options.loader ?? DEFAULT_LOADER;
  if (LOADERS.indexOf(loader) === -1) {
    throw new Error(`Invalid loader: ${loader}`);
  }
  return {
    name: "deno-loader",
    setup(build) {
      const cwd = build.initialOptions.absWorkingDir ?? Deno.cwd();

      let nodeModulesDir: string | null = null;
      if (options.nodeModulesDir) {
        nodeModulesDir = join(cwd, "node_modules");
      }

      let loaderImpl: Loader;

      const packageIdByNodeModules = new Map<string, string>();

      build.onStart(async function onStart() {
        packageIdByNodeModules.clear();
        switch (loader) {
          case "native":
            loaderImpl = new NativeLoader({
              infoOptions: {
                cwd,
                config: options.configPath,
                importMap: options.importMapURL,
                lock: options.lockPath,
                nodeModulesDir: options.nodeModulesDir,
              },
            });
            break;
          case "portable": {
            let lockPath: string | undefined = options.lockPath;
            if (lockPath === undefined && options.configPath !== undefined) {
              const config = await readDenoConfig(options.configPath);
              if (typeof config.lock === "string") {
                lockPath = join(dirname(options.configPath), config.lock);
              } else if (config.lock !== false) {
                lockPath = join(dirname(options.configPath), "deno.lock");
              }
            }
            loaderImpl = new PortableLoader({
              lock: lockPath,
            });
          }
        }
      });

      async function onResolve(
        args: esbuild.OnResolveArgs,
      ): Promise<esbuild.OnResolveResult | undefined> {
        if (isNodeModulesResolution(args)) {
          if (
            BUILTIN_NODE_MODULES.has(args.path) ||
            BUILTIN_NODE_MODULES.has("node:" + args.path)
          ) {
            return {
              path: args.path,
              external: true,
            };
          }
          if (nodeModulesDir) {
            return undefined;
          } else if (
            loaderImpl.nodeModulesDirForPackage &&
            loaderImpl.packageIdFromNameInPackage
          ) {
            let parentPackageId: string | undefined;
            let path = args.importer;
            while (true) {
              const packageId = packageIdByNodeModules.get(path);
              if (packageId) {
                parentPackageId = packageId;
                break;
              }
              const pathBefore = path;
              path = dirname(path);
              if (path === pathBefore) break;
            }
            if (!parentPackageId) {
              throw new Error(
                `Could not find package ID for importer: ${args.importer}`,
              );
            }
            if (args.path.startsWith(".")) {
              return undefined;
            } else {
              let packageName: string;
              let pathParts: string[];
              if (args.path.startsWith("@")) {
                const [scope, name, ...rest] = args.path.split("/");
                packageName = `${scope}/${name}`;
                pathParts = rest;
              } else {
                const [name, ...rest] = args.path.split("/");
                packageName = name;
                pathParts = rest;
              }
              const packageId = loaderImpl.packageIdFromNameInPackage(
                packageName,
                parentPackageId,
              );
              const id = packageId ?? parentPackageId;
              const resolveDir = await loaderImpl.nodeModulesDirForPackage(id);
              packageIdByNodeModules.set(resolveDir, id);
              const path = [packageName, ...pathParts].join("/");
              return await build.resolve(path, {
                kind: args.kind,
                resolveDir,
                importer: args.importer,
              });
            }
          } else {
            throw new Error(
              `To use "npm:" specifiers, you must specify "nodeModulesDir: true", or use "loader: native".`,
            );
          }
        }
        const specifier = esbuildResolutionToURL(args);

        // Once we have an absolute path, let the loader resolver figure out
        // what to do with it.
        const res = await loaderImpl.resolve(specifier);

        switch (res.kind) {
          case "esm": {
            const { specifier } = res;
            return urlToEsbuildResolution(specifier);
          }
          case "npm": {
            let resolveDir: string;
            if (nodeModulesDir) {
              resolveDir = nodeModulesDir;
            } else if (loaderImpl.nodeModulesDirForPackage) {
              resolveDir = await loaderImpl.nodeModulesDirForPackage(
                res.packageId,
              );
              packageIdByNodeModules.set(resolveDir, res.packageId);
            } else {
              throw new Error(
                `To use "npm:" specifiers, you must specify "nodeModulesDir: true", or use "loader: native".`,
              );
            }
            const path = `${res.packageName}${res.path ?? ""}`;
            return await build.resolve(path, {
              kind: args.kind,
              resolveDir,
              importer: args.importer,
            });
          }
          case "node": {
            return {
              path: res.path,
              external: true,
            };
          }
        }
      }
      build.onResolve({ filter: /.*/, namespace: "file" }, onResolve);
      build.onResolve({ filter: /.*/, namespace: "http" }, onResolve);
      build.onResolve({ filter: /.*/, namespace: "https" }, onResolve);
      build.onResolve({ filter: /.*/, namespace: "data" }, onResolve);
      build.onResolve({ filter: /.*/, namespace: "npm" }, onResolve);
      build.onResolve({ filter: /.*/, namespace: "jsr" }, onResolve);
      build.onResolve({ filter: /.*/, namespace: "node" }, onResolve);

      function onLoad(
        args: esbuild.OnLoadArgs,
      ): Promise<esbuild.OnLoadResult | null> | undefined {
        if (args.namespace === "file" && isInNodeModules(args.path)) {
          // inside node_modules, just let esbuild do it's thing
          return undefined;
        }
        const specifier = esbuildResolutionToURL(args);
        return loaderImpl.loadEsm(specifier);
      }
      // TODO(lucacasonato): once https://github.com/evanw/esbuild/pull/2968 is fixed, remove the catch all "file" handler
      build.onLoad({ filter: /.*/, namespace: "file" }, onLoad);
      build.onLoad({ filter: /.*/, namespace: "http" }, onLoad);
      build.onLoad({ filter: /.*/, namespace: "https" }, onLoad);
      build.onLoad({ filter: /.*/, namespace: "data" }, onLoad);
    },
  };
}
