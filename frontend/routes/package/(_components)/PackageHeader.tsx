// Copyright 2024 the JSR authors. All rights reserved. MIT license.
import { Package, PackageVersionWithUser } from "../../../utils/api_types.ts";
import { GitHub } from "../../../components/icons/GitHub.tsx";
import { RuntimeCompatIndicator } from "../../../components/RuntimeCompatIndicator.tsx";
import { getScoreTextColorClass } from "../../../utils/score_ring_color.ts";
import { CheckmarkStamp } from "../../../components/icons/CheckmarkStamp.tsx";
import { WarningTriangle } from "../../../components/icons/WarningTriangle.tsx";
import { Tooltip } from "../../../components/Tooltip.tsx";
import twas from "$twas";
import { parse } from "$std/semver/mod.ts";

interface PackageHeaderProps {
  package: Package;
  selectedVersion?: PackageVersionWithUser;
}

export function PackageHeader(
  { package: pkg, selectedVersion }: PackageHeaderProps,
) {
  const runtimeCompat = (
    <RuntimeCompatIndicator runtimeCompat={pkg.runtimeCompat} />
  );
  const isPrerelease = selectedVersion &&
    parse(selectedVersion.version).prerelease.length !== 0;

  return (
    <div class="space-y-6 mt-0 md:mt-4">
      {selectedVersion && pkg.latestVersion &&
        pkg.latestVersion !== selectedVersion.version && (
        <div class="border border-jsr-yellow-500 bg-jsr-yellow-50 rounded py-3 px-4 md:text-center">
          <div class="text-sm md:text-base flex items-center justify-center gap-4 md:gap-2">
            <WarningTriangle class="text-jsr-yellow-400 flex-none" />
            <span class="font-medium">
              This release {selectedVersion.yanked
                ? (
                  <>
                    was yanked — the latest stable version of @{pkg
                      .scope}/{pkg.name} is {pkg.latestVersion}.
                  </>
                )
                : isPrerelease && selectedVersion.newerVersionsCount == 0
                ? (
                  <>
                    is a pre-release — the latest stable version of @{pkg
                      .scope}/{pkg.name} is {pkg.latestVersion}.
                  </>
                )
                : (
                  <>
                    <span class="bold">
                      is {selectedVersion.newerVersionsCount}{" "}
                      version{selectedVersion.newerVersionsCount !== 1 && "s"}
                      {" "}
                      behind {pkg.latestVersion}
                    </span>{" "}
                    — the latest version of @{pkg.scope}/{pkg.name}.
                  </>
                )}{" "}
              <a
                class="link font-medium whitespace-nowrap"
                href={`/@${pkg.scope}/${pkg.name}`}
              >
                Jump to latest {(selectedVersion.yanked ||
                  (isPrerelease && selectedVersion.newerVersionsCount == 0)) &&
                  "stable"}
              </a>
            </span>
          </div>
        </div>
      )}

      <div class="flex flex-col md:flex-row items-start justify-between gap-6">
        <div class="space-y-3.5 flex-shrink">
          <div class="flex flex-row gap-x-3 gap-y-2 flex-wrap md:items-center">
            <h1 class="text-2xl md:text-3xl flex flex-wrap items-center font-sans gap-x-2">
              <div class="flex items-baseline gap-x-1">
                <span>
                  <a
                    href={`/@${pkg.scope}`}
                    class="link font-bold no-underline"
                  >
                    @{pkg.scope}
                  </a>/<span class="font-semibold">
                    {pkg.name}
                  </span>
                </span>

                {selectedVersion &&
                  (
                    <span class="text-lg md:text-[0.75em] font-bold">
                      <span class="relative text-[0.80em] -top-[0.175em] font-[800]">
                        @
                      </span>
                      {selectedVersion.version}
                    </span>
                  )}
              </div>

              {selectedVersion?.rekorLogId && (
                <Tooltip tooltip="Built and signed on GitHub Actions">
                  <CheckmarkStamp class="stroke-green-500 size-6" />
                </Tooltip>
              )}
            </h1>

            <div class="flex items-center gap-2">
              {selectedVersion &&
                pkg.latestVersion === selectedVersion?.version && (
                <div class="chip sm:big-chip bg-jsr-yellow-400 select-none">
                  latest
                </div>
              )}

              {selectedVersion?.yanked && (
                <div class="chip sm:big-chip bg-red-500 text-white select-none">
                  yanked
                </div>
              )}

              {pkg.githubRepository && (
                <a
                  class="chip sm:big-chip bg-jsr-gray-0 !inline-flex items-center gap-1 select-none"
                  href={`https://github.com/${pkg.githubRepository.owner}/${pkg.githubRepository.name}`}
                  target="_blank"
                  rel="noopener noreferrer"
                >
                  <GitHub class="text-black !size-4" />
                  <span>
                    {pkg.githubRepository.owner}/{pkg.githubRepository.name}
                  </span>
                </a>
              )}
            </div>
          </div>

          {pkg.description && (
            <p class="text-gray-600 max-w-3xl md:!mb-8">{pkg.description}</p>
          )}
        </div>

        <div class="flex flex-none md:items-end flex-col gap-2 md:gap-4 text-right pb-4">
          <div class="flex flex-col md:flex-row gap-2 md:gap-8 items-between">
            {runtimeCompat &&
              (
                <div class="flex flex-row md:flex-col items-center md:items-end gap-2 md:gap-1.5 text-sm font-bold">
                  <div>Works with</div>
                  {runtimeCompat}
                </div>
              )}

            {pkg.score !== null && (
              <a
                class="flex flex-row md:flex-col items-baseline md:items-end gap-2 md:gap-1.5 text-sm font-bold"
                href={`/@${pkg.scope}/${pkg.name}/score`}
              >
                <div>JSR Score</div>
                <div
                  class={`!leading-none md:text-xl ${
                    getScoreTextColorClass(pkg.score)
                  }`}
                >
                  {pkg.score}%
                </div>
              </a>
            )}
          </div>

          <div>
            {selectedVersion?.createdAt && (
              <div class="flex flex-row items-baseline md:items-end md:flex-col gap-2 md:gap-1.5 text-sm font-bold">
                <div>Published</div>
                <div
                  class="leading-none font-normal"
                  title={new Date(selectedVersion.createdAt).toISOString()
                    .slice(
                      0,
                      10,
                    )}
                >
                  {twas(new Date(selectedVersion.createdAt))}
                </div>
              </div>
            )}
          </div>
        </div>
      </div>
    </div>
  );
}
