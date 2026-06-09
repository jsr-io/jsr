// Copyright 2024 the JSR authors. All rights reserved. MIT license.
import type { Package } from "./api_types.ts";

/// The CI provider that built and (optionally) signed a package version.
/// New providers add an entry here and a matching row in PROVIDER_DISPLAY.
export type SourceProvider = "github";

export interface SourceProviderDisplay {
  /** Long-form label for the CI system, e.g. "GitHub Actions". */
  label: string;
  /** Short label for compact UI surfaces, e.g. "GitHub". */
  shortLabel: string;
}

export const PROVIDER_DISPLAY: Record<SourceProvider, SourceProviderDisplay> = {
  github: {
    label: "GitHub Actions",
    shortLabel: "GitHub",
  },
};

/// Resolve which source provider a package is currently linked to, if any.
/// Returns null when the package has no linked source repository.
export function getSourceProvider(pkg: Package): SourceProvider | null {
  if (pkg.githubRepository) return "github";
  return null;
}
