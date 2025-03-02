// Copyright 2024 the JSR authors. All rights reserved. MIT license.
import { Package, PackageVersionWithUser } from "../../../utils/api_types.ts";
import TbBrandGithub from "@preact-icons/tb/TbBrandGithub";
import { RuntimeCompatIndicator } from "../../../components/RuntimeCompatIndicator.tsx";
import { getScoreTextColorClass } from "../../../utils/score_ring_color.ts";
import {
  TbAlertTriangleFilled,
  TbExternalLink,
  TbFlag,
  TbRosetteDiscountCheck,
} from "@preact-icons/tb";
import { Tooltip } from "../../../components/Tooltip.tsx";
import twas from "twas";
import { greaterThan, parse } from "@std/semver";

interface PackageHeaderProps {
  package: Package;
  selectedVersion?: PackageVersionWithUser;
}

export function PackageHeader({
  package: pkg,
  selectedVersion,
}: PackageHeaderProps) {
  const runtimeCompat = (
    <RuntimeCompatIndicator runtimeCompat={pkg.runtimeCompat} />
  );

  const selectedVersionSemver = selectedVersion &&
    parse(selectedVersion.version);
  const isNewerPrerelease = selectedVersionSemver &&
    selectedVersionSemver.prerelease &&
    selectedVersionSemver.prerelease.length !== 0 &&
    (pkg.latestVersion === null ||
      greaterThan(selectedVersionSemver, parse(pkg.latestVersion)));

  const reportPackageBody = `Hello JSR team,
I would like to report a package for the following reason:
`;

  const mailLink = `mailto:report@jsr.io?subject=${
    encodeURIComponent(
      `Report package: ${pkg.scope}/${pkg.name}@${selectedVersion?.version}`,
    )
  }&body=${encodeURIComponent(reportPackageBody)}`;

  return (
    <div class="space-y-6 mt-0 md:mt-4">
      {pkg.isArchived && (
        <div class="rounded border border-red-300 bg-red-100 flex items-center justify-center p-4">
          This package has been archived, and as such it is read-only.
        </div>
      )}

      {selectedVersion && pkg.latestVersion &&
        pkg.latestVersion !== selectedVersion.version && (
        <div class="border border-jsr-yellow-500 bg-jsr-yellow-50 rounded py-3 px-4 md:text-center">
          <div class="text-sm md:text-base flex items-center justify-center gap-4 md:gap-2">
            <TbAlertTriangleFilled class="text-jsr-yellow-400 flex-none" />
            <span class="font-medium">
              This release {selectedVersion.yanked
                ? (
                  <>
                    was yanked — the latest version of @{pkg
                      .scope}/{pkg.name} is {pkg.latestVersion}.
                  </>
                )
                : isNewerPrerelease
                ? (
                  <>
                    is a pre-release — the latest non-prerelease version of
                    @{pkg.scope}/{pkg.name} is {pkg.latestVersion}.
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
                Jump to {isNewerPrerelease ? "this version " : "latest"}
              </a>
            </span>
          </div>
        </div>
      )}

      <div class="flex flex-col flex-wrap md:flex-row items-start justify-between gap-6">
        <div class="space-y-3.5 flex-shrink">
          <div class="flex flex-row gap-x-3 gap-y-2 flex-wrap md:items-center">
            <h1 class="text-2xl md:text-3xl flex flex-wrap items-center font-sans gap-x-2">
              <div class="flex items-baseline gap-x-1">
                <span>
                  <a
                    href={`/@${pkg.scope}`}
                    class="link font-bold pr-1 no-underline"
                    aria-label={`Scope: @${pkg.scope}`}
                  >
                    @{pkg.scope}
                  </a>/<a
                    href={`/@${pkg.scope}/${pkg.name}`}
                    class="link font-semibold no-underline"
                    aria-label={`Package: ${pkg.name}`}
                  >
                    {pkg.name}
                  </a>
                </span>

                {selectedVersion &&
                  (
                    <span
                      class="text-lg md:text-[0.75em] font-bold"
                      aria-label={`Version: ${selectedVersion.version}`}
                    >
                      <span class="relative text-[0.80em] -top-[0.175em] font-[800]">
                        @
                      </span>
                      {selectedVersion.version}
                    </span>
                  )}
              </div>

              {selectedVersion?.rekorLogId && (
                <Tooltip tooltip="Built and signed on GitHub Actions">
                  <TbRosetteDiscountCheck class="stroke-green-500 size-6" />
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
                  class="chip sm:big-chip bg-jsr-gray-100 !inline-flex items-center gap-1 select-none"
                  href={`https://github.com/${pkg.githubRepository.owner}/${pkg.githubRepository.name}`}
                  target="_blank"
                  rel="noopener noreferrer"
                  aria-label="GitHub repository"
                >
                  <TbBrandGithub
                    class="text-black !size-4"
                    aria-hidden
                  />
                  <span>
                    {pkg.githubRepository.owner}/{pkg.githubRepository.name}
                  </span>
                  <TbExternalLink strokeWidth="2.25" class="size-4" />
                </a>
              )}
            </div>
          </div>

          {pkg.description && (
            <p class="text-jsr-gray-600 max-w-3xl md:!mb-8">
              {pkg.description}
            </p>
          )}
        </div>

        <div class="flex flex-none md:items-end flex-col gap-2 md:gap-4 text-right pb-4 md:ml-auto">
          <div class="flex flex-col md:flex-row gap-2 md:gap-8 items-between">
            {runtimeCompat &&
              (
                <div class="flex flex-row md:flex-col items-center md:items-end gap-2 md:gap-1.5 text-sm font-bold">
                  <div aria-hidden="true">Works with</div>
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
                  {`${
                    twas(new Date(selectedVersion.createdAt).getTime())
                  } (${selectedVersion.version})`}
                </div>
              </div>
            )}
          </div>

          <div class="flex flex-row items-center gap-2">
            <a
              class="inline-flex items-center gap-1.5 text-sm bg-red-50 border border-red-300/30 rounded-md p-1.5 text-red-500 font-semibold hover:bg-red-100 focus:outline-none focus:border focus:border-red-300 focus:ring-1 focus:ring-red-300 focus:ring-opacity-50"
              href={mailLink}
            >
              <TbFlag class="size-4" /> Report package
            </a>
          </div>
        </div>
      </div>
    </div>
  );
}
