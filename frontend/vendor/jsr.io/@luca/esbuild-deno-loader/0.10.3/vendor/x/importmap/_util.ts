export interface SpecifierMap {
  [url: string]: string | null;
}
export interface Scopes {
  [url: string]: SpecifierMap;
}
export interface ImportMap {
  imports?: SpecifierMap;
  scopes?: Scopes;
}

export function isObject(object: unknown): object is Record<string, unknown> {
  return typeof object == "object" && object !== null &&
    object.constructor === Object;
}
export function sortObject(
  normalized: Record<string, unknown>,
): Record<string, unknown> {
  const sorted: Record<string, unknown> = {};
  const sortedKeys = Object.keys(normalized)
    .sort((a, b) => b.length - a.length);
  for (const key of sortedKeys) {
    sorted[key] = normalized[key];
  }
  return sorted;
}
export function isImportMap(importMap: unknown): importMap is ImportMap {
  return isObject(importMap) &&
    (importMap.imports !== undefined ? isImports(importMap.imports) : true) &&
    (importMap.scopes !== undefined ? isScopes(importMap.scopes) : true);
}
export function isImports(
  importsMap: unknown,
): importsMap is ImportMap {
  return isObject(importsMap);
}
export function isScopes(
  scopes: unknown,
): scopes is Scopes {
  return isObject(scopes) &&
    Object.values(scopes).every((value) => isSpecifierMap(value));
}
export function isSpecifierMap(
  specifierMap: unknown,
): specifierMap is SpecifierMap {
  return isObject(specifierMap);
}
export function isURL(url: unknown): boolean {
  try {
    new URL(url as string);
    return true;
  } catch {
    return false;
  }
}
