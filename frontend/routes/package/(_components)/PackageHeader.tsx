// Copyright 2024 the JSR authors. All rights reserved. MIT license.
import { Package, PackageVersionWithUser } from "../../../utils/api_types.ts";
import { GitHub } from "../../../components/icons/GitHub.tsx";
import { RuntimeCompatIndicator } from "../../../components/RuntimeCompatIndicator.tsx";
import { getScoreBgColorClass } from "../../../utils/score_ring_color.ts";

interface PackageHeaderProps {
  package: Package;
  selectedVersion?: PackageVersionWithUser;
}

export function PackageHeader(
  { package: pkg, selectedVersion }: PackageHeaderProps,
) {
  return (
    <div class="space-y-2.5 mt-0 md:mt-4">
      <div class="flex flex-wrap items-center justify-between gap-2">
        <div class="flex flex-row gap-x-3 gap-y-2 flex-wrap md:items-center">
          <h1 class="text-2xl md:text-3xl flex flex-wrap items-baseline font-sans gap-x-2">
            <span>
              <a href={`/@${pkg.scope}`} class="link font-bold no-underline">
                @{pkg.scope}
              </a>/<span class="font-semibold">
                {pkg.name}
              </span>
            </span>
            {selectedVersion &&
              (
                <span class="text-lg md:text-[0.75em] font-bold">
                  <span class="relative text-[0.85em] -top-[0.175em] font-[800]">
                    @
                  </span>
                  {selectedVersion.version}
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
          {pkg.score !== null && (
            <a
              href={`/@${pkg.scope}/${pkg.name}/score`}
              class="flex items-center gap-2 select-none text-sm font-medium"
            >
              <span class="max-sm:hidden">Score</span>
              <div
                class={`flex w-12 h-12 items-center justify-center aspect-square rounded-full p-1 ${
                  getScoreBgColorClass(pkg.score)
                }`}
                style={`background-image: conic-gradient(transparent, transparent ${pkg.score}%, #e7e8e8 ${pkg.score}%)`}
              >
                <span class="rounded-full w-full h-full bg-white flex justify-center items-center text-center font-bold">
                  {pkg.score}%
                </span>
              </div>
            </a>
          )}

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
