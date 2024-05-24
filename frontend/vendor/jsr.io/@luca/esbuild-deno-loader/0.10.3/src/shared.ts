import { extname, fromFileUrl, SEPARATOR, toFileUrl } from "jsr:@std/path@0.213";
import * as JSONC from "jsr:@std/jsonc@0.213";
import { ImportMap } from "../vendor/x/importmap/mod.ts";
import { MediaType } from "./deno.ts";
import type * as esbuild from "./esbuild_types.ts";

export interface Loader {
  resolve(specifier: URL): Promise<LoaderResolution>;
  loadEsm(specifier: URL): Promise<esbuild.OnLoadResult>;

  packageIdFromNameInPackage?(
    name: string,
    parentPackageId: string,
  ): string | null;
  nodeModulesDirForPackage?(npmPackageId?: string): Promise<string>;
}

export type LoaderResolution =
  | LoaderResolutionEsm
  | LoaderResolutionNpm
  | LoaderResolutionNode;

export interface LoaderResolutionEsm {
  kind: "esm";
  specifier: URL;
}

export interface LoaderResolutionNpm {
  kind: "npm";
  packageId: string;
  packageName: string;
  path: string;
}

export interface LoaderResolutionNode {
  kind: "node";
  path: string;
}

export function mediaTypeToLoader(mediaType: MediaType): esbuild.Loader {
  switch (mediaType) {
    case "JavaScript":
    case "Mjs":
      return "js";
    case "JSX":
      return "jsx";
    case "TypeScript":
    case "Mts":
      return "ts";
    case "TSX":
      return "tsx";
    case "Json":
      return "json";
    default:
      throw new Error(`Unhandled media type ${mediaType}.`);
  }
}

/** Esbuild's representation of a module specifier. */
export interface EsbuildResolution {
  /** The namespace, like `file`, `https`, or `npm`. */
  namespace: string;
  /** The path. When the namespace is `file`, this is a file path. Otherwise
   * this is everything in a URL with the namespace as the scheme, after the
   * `:` of the scheme. */
  path: string;
}

/**
 * Turn a URL into an {@link EsbuildResolution} by splitting the URL into a
 * namespace and path.
 *
 * For file URLs, the path returned is a file path not a URL path representing a
 * file.
 */
export function urlToEsbuildResolution(url: URL): EsbuildResolution {
  if (url.protocol === "file:") {
    return { path: fromFileUrl(url), namespace: "file" };
  }

  const namespace = url.protocol.slice(0, -1);
  const path = url.href.slice(namespace.length + 1);
  return { path, namespace };
}

/**
 * Turn an {@link EsbuildResolution} into a URL by joining the namespace and
 * path into a URL string.
 *
 * For file URLs, the path is interpreted as a file path not as a URL path
 * representing a file.
 */
export function esbuildResolutionToURL(specifier: EsbuildResolution): URL {
  if (specifier.namespace === "file") {
    return toFileUrl(specifier.path);
  }

  return new URL(`${specifier.namespace}:${specifier.path}`);
}

interface DenoConfig {
  imports?: unknown;
  scopes?: unknown;
  lock?: boolean | string;
  importMap?: string;
}

export async function readDenoConfig(path: string): Promise<DenoConfig> {
  const file = await Deno.readTextFile(path);
  const res = JSONC.parse(file);
  if (typeof res !== "object" || res === null || Array.isArray(res)) {
    throw new Error(`Deno config at ${path} must be an object`);
  }
  if (
    "imports" in res &&
    (typeof res.imports !== "object" || res.imports === null ||
      Array.isArray(res.imports))
  ) {
    throw new Error(`Deno config at ${path} has invalid "imports" key`);
  }
  if (
    "scopes" in res &&
    (typeof res.scopes !== "object" || res.scopes === null ||
      Array.isArray(res.scopes))
  ) {
    throw new Error(`Deno config at ${path} has invalid "scopes" key`);
  }
  if (
    "lock" in res &&
    typeof res.lock !== "boolean" && typeof res.lock !== "string"
  ) {
    throw new Error(`Deno config at ${path} has invalid "lock" key`);
  }
  if ("importMap" in res && typeof res.importMap !== "string") {
    throw new Error(`Deno config at ${path} has invalid "importMap" key`);
  }
  return res;
}

export function mapContentType(
  specifier: URL,
  contentType: string | null,
): MediaType {
  if (contentType !== null) {
    const contentTypes = contentType.split(";");
    const mediaType = contentTypes[0].toLowerCase();
    switch (mediaType) {
      case "application/typescript":
      case "text/typescript":
      case "video/vnd.dlna.mpeg-tts":
      case "video/mp2t":
      case "application/x-typescript":
        return mapJsLikeExtension(specifier, "TypeScript");
      case "application/javascript":
      case "text/javascript":
      case "application/ecmascript":
      case "text/ecmascript":
      case "application/x-javascript":
      case "application/node":
        return mapJsLikeExtension(specifier, "JavaScript");
      case "text/jsx":
        return "JSX";
      case "text/tsx":
        return "TSX";
      case "application/json":
      case "text/json":
        return "Json";
      case "application/wasm":
        return "Wasm";
      case "text/plain":
      case "application/octet-stream":
        return mediaTypeFromSpecifier(specifier);
      default:
        return "Unknown";
    }
  } else {
    return mediaTypeFromSpecifier(specifier);
  }
}

function mapJsLikeExtension(
  specifier: URL,
  defaultType: MediaType,
): MediaType {
  const path = specifier.pathname;
  switch (extname(path)) {
    case ".jsx":
      return "JSX";
    case ".mjs":
      return "Mjs";
    case ".cjs":
      return "Cjs";
    case ".tsx":
      return "TSX";
    case ".ts":
      if (path.endsWith(".d.ts")) {
        return "Dts";
      } else {
        return defaultType;
      }
    case ".mts": {
      if (path.endsWith(".d.mts")) {
        return "Dmts";
      } else {
        return defaultType == "JavaScript" ? "Mjs" : "Mts";
      }
    }
    case ".cts": {
      if (path.endsWith(".d.cts")) {
        return "Dcts";
      } else {
        return defaultType == "JavaScript" ? "Cjs" : "Cts";
      }
    }
    default:
      return defaultType;
  }
}

function mediaTypeFromSpecifier(specifier: URL): MediaType {
  const path = specifier.pathname;
  switch (extname(path)) {
    case "":
      if (path.endsWith("/.tsbuildinfo")) {
        return "TsBuildInfo";
      } else {
        return "Unknown";
      }
    case ".ts":
      if (path.endsWith(".d.ts")) {
        return "Dts";
      } else {
        return "TypeScript";
      }
    case ".mts":
      if (path.endsWith(".d.mts")) {
        return "Dmts";
      } else {
        return "Mts";
      }
    case ".cts":
      if (path.endsWith(".d.cts")) {
        return "Dcts";
      } else {
        return "Cts";
      }
    case ".tsx":
      return "TSX";
    case ".js":
      return "JavaScript";
    case ".jsx":
      return "JSX";
    case ".mjs":
      return "Mjs";
    case ".cjs":
      return "Cjs";
    case ".json":
      return "Json";
    case ".wasm":
      return "Wasm";
    case ".tsbuildinfo":
      return "TsBuildInfo";
    case ".map":
      return "SourceMap";
    default:
      return "Unknown";
  }
}

export interface NpmSpecifier {
  name: string;
  version: string | null;
  path: string | null;
}

export function parseNpmSpecifier(specifier: URL): NpmSpecifier {
  if (specifier.protocol !== "npm:") throw new Error("Invalid npm specifier");
  const path = specifier.pathname;
  const startIndex = path[0] === "/" ? 1 : 0;
  let pathStartIndex;
  let versionStartIndex;
  if (path[startIndex] === "@") {
    const firstSlash = path.indexOf("/", startIndex);
    if (firstSlash === -1) {
      throw new Error(`Invalid npm specifier: ${specifier}`);
    }
    pathStartIndex = path.indexOf("/", firstSlash + 1);
    versionStartIndex = path.indexOf("@", firstSlash + 1);
  } else {
    pathStartIndex = path.indexOf("/", startIndex);
    versionStartIndex = path.indexOf("@", startIndex);
  }

  if (pathStartIndex === -1) pathStartIndex = path.length;
  if (versionStartIndex === -1) versionStartIndex = path.length;

  if (versionStartIndex > pathStartIndex) {
    versionStartIndex = pathStartIndex;
  }

  if (startIndex === versionStartIndex) {
    throw new Error(`Invalid npm specifier: ${specifier}`);
  }

  return {
    name: path.slice(startIndex, versionStartIndex),
    version: versionStartIndex === pathStartIndex
      ? null
      : path.slice(versionStartIndex + 1, pathStartIndex),
    path: pathStartIndex === path.length ? null : path.slice(pathStartIndex),
  };
}

export interface JsrSpecifier {
  name: string;
  version: string | null;
  path: string | null;
}

export function parseJsrSpecifier(specifier: URL): JsrSpecifier {
  if (specifier.protocol !== "jsr:") throw new Error("Invalid jsr specifier");
  const path = specifier.pathname;
  const startIndex = path[0] === "/" ? 1 : 0;
  if (path[startIndex] !== "@") {
    throw new Error(`Invalid jsr specifier: ${specifier}`);
  }
  const firstSlash = path.indexOf("/", startIndex);
  if (firstSlash === -1) {
    throw new Error(`Invalid jsr specifier: ${specifier}`);
  }
  let pathStartIndex = path.indexOf("/", firstSlash + 1);
  let versionStartIndex = path.indexOf("@", firstSlash + 1);

  if (pathStartIndex === -1) pathStartIndex = path.length;
  if (versionStartIndex === -1) versionStartIndex = path.length;

  if (versionStartIndex > pathStartIndex) {
    versionStartIndex = pathStartIndex;
  }

  if (startIndex === versionStartIndex) {
    throw new Error(`Invalid jsr specifier: ${specifier}`);
  }

  return {
    name: path.slice(startIndex, versionStartIndex),
    version: versionStartIndex === pathStartIndex
      ? null
      : path.slice(versionStartIndex + 1, pathStartIndex),
    path: pathStartIndex === path.length ? null : path.slice(pathStartIndex),
  };
}

// For all pairs in `imports` where the specifier does not end in a /, and the
// target starts with `jsr:` or `npm:`, and no entry exists for `${specifier}/`,
// add an entry for `${specifier}/` pointing to the target with a / appended,
// and a `/` appended to the scheme, if none is present there.
export function expandEmbeddedImportMap(importMap: ImportMap) {
  if (importMap.imports !== undefined) {
    const newImports: [string, string | null][] = [];
    for (const [specifier, target] of Object.entries(importMap.imports)) {
      newImports.push([specifier, target]);
      if (
        !specifier.endsWith("/") && target &&
        (target.startsWith("jsr:") || target.startsWith("npm:")) &&
        !importMap.imports[specifier + "/"]
      ) {
        const newSpecifier = specifier + "/";
        const newTarget = target.slice(0, 4) + "/" +
          target.slice(target[4] === "/" ? 5 : 4) + "/";
        newImports.push([newSpecifier, newTarget]);
      }
    }
    importMap.imports = Object.fromEntries(newImports);
  }
}

const SLASH_NODE_MODULES_SLASH = `${SEPARATOR}node_modules${SEPARATOR}`;
const SLASH_NODE_MODULES = `${SEPARATOR}node_modules`;

export function isInNodeModules(path: string): boolean {
  return path.includes(SLASH_NODE_MODULES_SLASH) ||
    path.endsWith(SLASH_NODE_MODULES);
}

export function isNodeModulesResolution(args: esbuild.OnResolveArgs) {
  return (
    (args.namespace === "" || args.namespace === "file") &&
    (isInNodeModules(args.resolveDir) || isInNodeModules(args.path) ||
      isInNodeModules(args.importer))
  );
}
