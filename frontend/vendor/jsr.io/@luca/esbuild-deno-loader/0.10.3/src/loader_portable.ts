import type * as esbuild from "./esbuild_types.ts";
import { fromFileUrl } from "jsr:@std/path@0.213";
import * as deno from "./deno.ts";
import {
  Loader,
  LoaderResolution,
  mapContentType,
  mediaTypeToLoader,
  parseJsrSpecifier,
  parseNpmSpecifier,
} from "./shared.ts";

interface Module {
  specifier: string;
  mediaType: deno.MediaType;
  data: Uint8Array;
}

const JSR_REGISTRY_URL = Deno.env.get("DENO_REGISTRY_URL") ?? "https://jsr.io";

async function readLockfile(path: string): Promise<deno.Lockfile | null> {
  try {
    const data = await Deno.readTextFile(path);
    return JSON.parse(data);
  } catch (err) {
    if (err instanceof Deno.errors.NotFound) {
      return null;
    }
    throw err;
  }
}

interface PortableLoaderOptions {
  lock?: string;
}

export class PortableLoader implements Loader {
  #options: PortableLoaderOptions;
  #fetchOngoing = new Map<string, Promise<void>>();
  #lockfile: Promise<deno.Lockfile | null> | deno.Lockfile | null | undefined;

  #fetchModules = new Map<string, Module>();
  #fetchRedirects = new Map<string, string>();

  constructor(options: PortableLoaderOptions) {
    this.#options = options;
  }

  async resolve(specifier: URL): Promise<LoaderResolution> {
    switch (specifier.protocol) {
      case "file:": {
        return { kind: "esm", specifier };
      }
      case "http:":
      case "https:":
      case "data:": {
        const module = await this.#loadRemote(specifier.href);
        return { kind: "esm", specifier: new URL(module.specifier) };
      }
      case "npm:": {
        const npmSpecifier = parseNpmSpecifier(specifier);
        return {
          kind: "npm",
          packageId: "",
          packageName: npmSpecifier.name,
          path: npmSpecifier.path ?? "",
        };
      }
      case "node:": {
        return { kind: "node", path: specifier.pathname };
      }
      case "jsr:": {
        const resolvedSpecifier = await this.#resolveJsrSpecifier(specifier);
        return { kind: "esm", specifier: resolvedSpecifier };
      }
      default:
        throw new Error(`Unsupported scheme: '${specifier.protocol}'`);
    }
  }

  async #resolveJsrSpecifier(specifier: URL): Promise<URL> {
    // parse the JSR specifier.
    const jsrSpecifier = parseJsrSpecifier(specifier);

    // Attempt to load the lockfile.
    if (this.#lockfile === undefined) {
      this.#lockfile = typeof this.#options.lock === "string"
        ? readLockfile(this.#options.lock)
        : null;
    }
    if (this.#lockfile instanceof Promise) {
      this.#lockfile = await this.#lockfile;
    }
    if (this.#lockfile === null) {
      throw new Error(
        "jsr: specifiers are not supported in the portable loader without a lockfile",
      );
    }
    const lockfile = this.#lockfile;
    if (lockfile.version !== "3") {
      throw new Error(
        "Unsupported lockfile version: " + lockfile.version,
      );
    }

    // Look up the package + constraint in the lockfile.
    const id = `jsr:${jsrSpecifier.name}${
      jsrSpecifier.version ? `@${jsrSpecifier.version}` : ""
    }`;
    const lockfileEntry = lockfile.packages?.specifiers?.[id];
    if (!lockfileEntry) {
      throw new Error(`Specifier not found in lockfile: ${id}`);
    }
    const lockfileEntryParsed = parseJsrSpecifier(new URL(lockfileEntry));

    // Load the JSR manifest to find the export path.
    const manifestUrl = new URL(
      `./${lockfileEntryParsed.name}/${lockfileEntryParsed
        .version!}_meta.json`,
      JSR_REGISTRY_URL,
    );
    const manifest = await this.#loadRemote(manifestUrl.href);
    if (manifest.mediaType !== "Json") {
      throw new Error(
        `Expected JSON media type for JSR manifest, got: ${manifest.mediaType}`,
      );
    }
    const manifestData = new TextDecoder().decode(manifest.data);
    const manifestJson = JSON.parse(manifestData);

    // Look up the export path in the manifest.
    const exportEntry = `.${jsrSpecifier.path ?? ""}`;
    const exportPath = manifestJson.exports[exportEntry];
    if (!exportPath) {
      throw new Error(
        `Package '${lockfileEntry}' has no export named '${exportEntry}'`,
      );
    }

    // Return the resolved URL.
    return new URL(
      `./${lockfileEntryParsed.name}/${lockfileEntryParsed
        .version!}/${exportPath}`,
      JSR_REGISTRY_URL,
    );
  }

  async loadEsm(url: URL): Promise<esbuild.OnLoadResult> {
    let module: Module;
    switch (url.protocol) {
      case "file:": {
        module = await this.#loadLocal(url);
        break;
      }
      case "http:":
      case "https:":
      case "data:": {
        module = await this.#loadRemote(url.href);
        break;
      }
      default:
        throw new Error("[unreachable] unsupported esm scheme " + url.protocol);
    }

    const loader = mediaTypeToLoader(module.mediaType);

    const res: esbuild.OnLoadResult = { contents: module.data, loader };
    if (url.protocol === "file:") {
      res.watchFiles = [fromFileUrl(module.specifier)];
    }
    return res;
  }

  #resolveRemote(specifier: string): string {
    return this.#fetchRedirects.get(specifier) ?? specifier;
  }

  async #loadRemote(specifier: string): Promise<Module> {
    for (let i = 0; i < 10; i++) {
      specifier = this.#resolveRemote(specifier);
      const module = this.#fetchModules.get(specifier);
      if (module) return module;

      let promise = this.#fetchOngoing.get(specifier);
      if (!promise) {
        promise = this.#fetch(specifier);
        this.#fetchOngoing.set(specifier, promise);
      }

      await promise;
    }

    throw new Error("Too many redirects. Last one: " + specifier);
  }

  async #fetch(specifier: string): Promise<void> {
    const resp = await fetch(specifier, {
      redirect: "manual",
    });
    if (resp.status < 200 && resp.status >= 400) {
      throw new Error(
        `Encountered status code ${resp.status} while fetching ${specifier}.`,
      );
    }

    if (resp.status >= 300 && resp.status < 400) {
      await resp.body?.cancel();
      const location = resp.headers.get("location");
      if (!location) {
        throw new Error(
          `Redirected without location header while fetching ${specifier}.`,
        );
      }

      const url = new URL(location, specifier);
      if (url.protocol !== "https:" && url.protocol !== "http:") {
        throw new Error(
          `Redirected to unsupported protocol '${url.protocol}' while fetching ${specifier}.`,
        );
      }

      this.#fetchRedirects.set(specifier, url.href);
      return;
    }

    const contentType = resp.headers.get("content-type");
    const mediaType = mapContentType(new URL(specifier), contentType);

    const data = new Uint8Array(await resp.arrayBuffer());
    this.#fetchModules.set(specifier, {
      specifier,
      mediaType,
      data,
    });
  }

  async #loadLocal(specifier: URL): Promise<Module> {
    const path = fromFileUrl(specifier);

    const mediaType = mapContentType(specifier, null);
    const data = await Deno.readFile(path);

    return { specifier: specifier.href, mediaType, data };
  }
}
