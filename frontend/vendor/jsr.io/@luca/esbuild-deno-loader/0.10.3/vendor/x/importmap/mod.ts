import {
  ImportMap,
  isImportMap,
  isImports,
  isScopes,
  isSpecifierMap,
  isURL,
  Scopes,
  sortObject,
  SpecifierMap,
} from "./_util.ts";

export type { ImportMap } from "./_util.ts";

/* https://wicg.github.io/import-maps/#sort-and-normalize-a-specifier-map */
function sortAndNormalizeSpecifierMap(
  originalMap: SpecifierMap,
  baseURL: URL,
): SpecifierMap {
  const normalized: SpecifierMap = {};
  for (const [specifierKey, value] of Object.entries(originalMap)) {
    const normalizedSpecifierKey = normalizeSpecifierKey(specifierKey, baseURL);
    if (normalizedSpecifierKey === null) continue;
    if (typeof value !== "string") {
      console.warn(`addresses need to be strings.`);
      normalized[normalizedSpecifierKey] = null;
      continue;
    }
    const addressURL = parseUrlLikeImportSpecifier(value, baseURL);

    if (addressURL === null) {
      console.warn(`the address was invalid.`);
      normalized[normalizedSpecifierKey] = null;
      continue;
    }
    if (specifierKey.endsWith("/") && !serializeURL(addressURL).endsWith("/")) {
      console.warn(
        `an invalid address was given for the specifier key specifierKey; since specifierKey ended in a slash, the address needs to as well.`,
      );
      normalized[normalizedSpecifierKey] = null;
      continue;
    }
    normalized[normalizedSpecifierKey] = serializeURL(addressURL);
  }
  return sortObject(normalized) as SpecifierMap;
}
/* https://url.spec.whatwg.org/#concept-url-serializer */
function serializeURL(url: URL): string {
  return url.href;
}
/* https://wicg.github.io/import-maps/#sort-and-normalize-scopes */
function sortAndNormalizeScopes(
  originalMap: Scopes,
  baseURL: URL,
): Scopes {
  const normalized: Scopes = {};
  for (
    const [scopePrefix, potentialSpecifierMap] of Object.entries(originalMap)
  ) {
    if (!isSpecifierMap(potentialSpecifierMap)) {
      throw new TypeError(
        `the value of the scope with prefix scopePrefix needs to be an object.`,
      );
    }
    let scopePrefixURL;
    try {
      scopePrefixURL = new URL(scopePrefix, baseURL);
    } catch {
      console.warn(`the scope prefix URL was not parseable.`);
      continue;
    }
    const normalizedScopePrefix = serializeURL(scopePrefixURL);
    normalized[normalizedScopePrefix] = sortAndNormalizeSpecifierMap(
      potentialSpecifierMap,
      baseURL,
    );
  }

  const sorted: Scopes = {};
  for (const key of Object.keys(normalized)) {
    sorted[key] = sortObject(normalized[key]) as SpecifierMap;
  }
  return sortObject(sorted) as Scopes;
}
/* https://wicg.github.io/import-maps/#normalize-a-specifier-key */
function normalizeSpecifierKey(
  specifierKey: string,
  baseURL: URL,
): string | null {
  if (!specifierKey.length) {
    console.warn("specifier key cannot be an empty string.");
    return null;
  }
  const url = parseUrlLikeImportSpecifier(specifierKey, baseURL);
  if (url !== null) {
    return serializeURL(url);
  }
  return specifierKey;
}
/* https://wicg.github.io/import-maps/#parse-a-url-like-import-specifier */
function parseUrlLikeImportSpecifier(
  specifier: string,
  baseURL: URL,
): URL | null {
  if (
    baseURL && (specifier.startsWith("/") ||
      specifier.startsWith("./") ||
      specifier.startsWith("../"))
  ) {
    try {
      const url = new URL(specifier, baseURL);
      return url;
    } catch {
      return null;
    }
  }

  try {
    const url = new URL(specifier);
    return url;
  } catch {
    return null;
  }
}

const specialSchemes = [
  "ftp",
  "file",
  "http",
  "https",
  "ws",
  "wss",
];
/* https://url.spec.whatwg.org/#is-special */
function isSpecial(asURL: URL): boolean {
  return specialSchemes.some((scheme) =>
    serializeURL(asURL).startsWith(scheme)
  );
}
/* https://wicg.github.io/import-maps/#resolve-an-imports-match */
function resolveImportsMatch(
  normalizedSpecifier: string,
  asURL: URL | null,
  specifierMap: SpecifierMap,
): string | null {
  for (
    const [specifierKey, resolutionResult] of Object.entries(specifierMap)
  ) {
    if (specifierKey === normalizedSpecifier) {
      if (resolutionResult === null) {
        throw new TypeError(
          `resolution of specifierKey was blocked by a null entry.`,
        );
      }
      if (!isURL(resolutionResult)) {
        throw new TypeError(`resolutionResult must be an URL.`);
      }
      return resolutionResult;
    } else if (
      specifierKey.endsWith("/") &&
      normalizedSpecifier.startsWith(specifierKey) &&
      (asURL === null || isSpecial(asURL))
    ) {
      if (resolutionResult === null) {
        throw new TypeError(
          `resolution of specifierKey was blocked by a null entry.`,
        );
      }
      if (!isURL(resolutionResult)) {
        throw new TypeError(`resolutionResult must be an URL.`);
      }
      const afterPrefix = normalizedSpecifier.slice(specifierKey.length);

      if (!resolutionResult.endsWith("/")) {
        throw new TypeError(`resolutionResult does not end with "/"`);
      }

      try {
        const url = new URL(afterPrefix, resolutionResult);
        if (!isURL(url)) {
          throw new TypeError(`url must be an URL.`);
        }
        if (!serializeURL(url).startsWith(resolutionResult)) {
          throw new TypeError(
            `resolution of normalizedSpecifier was blocked due to it backtracking above its prefix specifierKey.`,
          );
        }
        return serializeURL(url);
      } catch {
        throw new TypeError(
          `resolution of normalizedSpecifier was blocked since the afterPrefix portion could not be URL-parsed relative to the resolutionResult mapped to by the specifierKey prefix.`,
        );
      }
    }
  }
  return null;
}
/* https://wicg.github.io/import-maps/#parsing */
// do not parse JSON string as done in the specs. That can be done with JSON.parse
export function resolveImportMap(
  importMap: ImportMap,
  baseURL: URL,
): ImportMap {
  let sortedAndNormalizedImports = {};
  if (!isImportMap(importMap)) {
    throw new TypeError(`the top-level value needs to be a JSON object.`);
  }
  const { imports, scopes } = importMap;
  if (imports !== undefined) {
    if (!isImports(imports)) {
      throw new TypeError(`"imports" top-level key needs to be an object.`);
    }
    sortedAndNormalizedImports = sortAndNormalizeSpecifierMap(
      imports,
      baseURL,
    );
  }
  let sortedAndNormalizedScopes = {};
  if (scopes !== undefined) {
    if (!isScopes(scopes)) {
      throw new TypeError(`"scopes" top-level key needs to be an object.`);
    }
    sortedAndNormalizedScopes = sortAndNormalizeScopes(
      scopes,
      baseURL,
    );
  }
  if (
    Object.keys(importMap).find((key) => key !== "imports" && key !== "scopes")
  ) {
    console.warn(`an invalid top-level key was present in the import map.`);
  }
  return {
    imports: sortedAndNormalizedImports,
    scopes: sortedAndNormalizedScopes,
  };
}
/* https://wicg.github.io/import-maps/#new-resolve-algorithm */
export function resolveModuleSpecifier(
  specifier: string,
  { imports = {}, scopes = {} }: ImportMap,
  baseURL: URL,
): string {
  const baseURLString = serializeURL(baseURL);
  const asURL = parseUrlLikeImportSpecifier(specifier, baseURL);
  const normalizedSpecifier = asURL !== null ? serializeURL(asURL) : specifier;
  for (const [scopePrefix, scopeImports] of Object.entries(scopes)) {
    if (
      scopePrefix === baseURLString ||
      (scopePrefix.endsWith("/") && baseURLString.startsWith(scopePrefix))
    ) {
      const scopeImportsMatch = resolveImportsMatch(
        normalizedSpecifier,
        asURL,
        scopeImports,
      );
      if (scopeImportsMatch !== null) {
        return scopeImportsMatch;
      }
    }
  }

  const topLevelImportsMatch = resolveImportsMatch(
    normalizedSpecifier,
    asURL,
    imports,
  );

  if (topLevelImportsMatch !== null) {
    return topLevelImportsMatch;
  }

  if (asURL !== null) {
    return serializeURL(asURL);
  }
  throw new TypeError(
    `specifier was a bare specifier, but was not remapped to anything by importMap.`,
  );
}
