// Copyright 2024 the JSR authors. All rights reserved. MIT license.
import { Package, PackageVersionWithUser } from "../../../utils/api_types.ts";
import { GitHub } from "../../../components/icons/GitHub.tsx";
import { RuntimeCompatIndicator } from "../../../components/RuntimeCompatIndicator.tsx";
import { QuotaUsage } from "../../../components/QuotaCard.tsx";

interface PackageHeaderProps {
  package: Package;
  selectedVersion?: PackageVersionWithUser;
}

export function PackageHeader(
  { package: pkg, selectedVersion }: PackageHeaderProps,
) {
  return (
    <div class="space-y-2.5 mt-0 md:mt-4">
      <div class="flex items-center justify-between">
        <div class="flex flex-row gap-3 flex-wrap md:items-center">
          <h1 class="text-2xl md:text-3xl flex flex-wrap items-baseline font-sans">
            <a href={`/@${pkg.scope}`} class="link font-bold">
              @{pkg.scope}
            </a>/<span class="font-semibold">
              {pkg.name}
            </span>
            {selectedVersion &&
              (
                <span class="text-lg md:text-xl font-bold ml-2">
                  @{selectedVersion.version}
                </span>
              )}
          </h1>
          <div class="flex items-center gap-1">
            {selectedVersion &&
              pkg.latestVersion === selectedVersion?.version && (
              <div class="chip sm:big-chip bg-jsr-yellow-400">
                latest
              </div>
            )}
            {selectedVersion?.yanked && (
              <div class="chip sm:big-chip bg-red-500 text-white">
                yanked
              </div>
            )}
          </div>
        </div>
        <div class="flex items-center gap-8">
          <a
            href={`/@${pkg.scope}/${pkg.name}/score`}
            class="flex items-center gap-2 select-none"
          >
            <span>score</span>
            <div class="rounded-full ring-1 ring-jsr-cyan-950 bg-jsr-cyan-200 size-12 text-center leading-[3rem]">
              <span class="font-bold">{pkg.score}</span>
              <span class="text-xs">%</span>
            </div>
          </a>

          {selectedVersion && pkg.latestVersion !== selectedVersion.version && (
            <a class="button-primary" href={`/@${pkg.scope}/${pkg.name}`}>
              Jump To Latest
            </a>
          )}
        </div>
      </div>
      {pkg.description && (
        <p class="text-gray-600 max-w-3xl">{pkg.description}</p>
      )}
      {pkg.githubRepository && (
        <a
          class="link inline-flex items-center gap-1.5 text-sm"
          href={`https://github.com/${pkg.githubRepository.owner}/${pkg.githubRepository.name}`}
        >
          <GitHub class="text-gray-500 h-4 w-4" />
          Repository
        </a>
      )}
      <RuntimeCompatIndicator runtimeCompat={pkg.runtimeCompat} labeled />
    </div>
  );
}
