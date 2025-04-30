// Copyright 2024 the JSR authors. All rights reserved. MIT license.
import { HttpError, RouteConfig } from "fresh";
import type {
  PackageVersionWithUser,
  PublishingTask,
  PublishingTaskStatus,
} from "../../utils/api_types.ts";
import { define } from "../../util.ts";
import { compare, equals, format, lessThan, parse, SemVer } from "@std/semver";
import { timeAgo } from "../../utils/timeAgo.ts";
import { packageData } from "../../utils/data.ts";
import { PackageHeader } from "./(_components)/PackageHeader.tsx";
import { PackageNav, Params } from "./(_components)/PackageNav.tsx";
import { path } from "../../utils/api.ts";
import { TbAlertCircle, TbCheck, TbClockHour3, TbTrashX } from "tb-icons";
import { ScopeIAM, scopeIAM } from "../../utils/iam.ts";
import { DownloadChart } from "./(_islands)/DownloadChart.tsx";

export default define.page<typeof handler>(function Versions({
  data,
  params,
  state,
}) {
  const iam = scopeIAM(state, data.member);

  const latestVersionInReleaseTrack: Record<string, SemVer> = {};

  const versions = new Map<
    string,
    {
      semver: SemVer;
      releaseTrack: string;
      version: PackageVersionWithUser | null;
      tasks: PublishingTask[];
    }
  >();

  for (const version of data.versions) {
    const semver = parse(version.version);
    const releaseTrack = `${semver.major}${
      semver.major === 0 ? `.${semver.minor}` : ""
    }.x`;
    versions.set(version.version, {
      semver,
      releaseTrack,
      version,
      tasks: [],
    });
    if (version.yanked) continue;
    if (
      (!semver.prerelease || semver.prerelease.length === 0) &&
      (latestVersionInReleaseTrack[releaseTrack] === undefined ||
        lessThan(latestVersionInReleaseTrack[releaseTrack], semver))
    ) {
      latestVersionInReleaseTrack[releaseTrack] = semver;
    }
  }

  if (data.publishingTasks) {
    for (const publishingTask of data.publishingTasks) {
      let version = versions.get(publishingTask.packageVersion);
      if (!version) {
        const semver = parse(publishingTask.packageVersion);
        const releaseTrack = `${semver.major}.${
          semver.major === 0 ? `${semver.minor}.` : ""
        }.x`;
        version = {
          semver,
          releaseTrack,
          version: null,
          tasks: [],
        };
        versions.set(publishingTask.packageVersion, version);
      }
      version.tasks.push(publishingTask);
    }
  }

  const versionsArray = Array.from(versions.values())
    .sort((a, b) => compare(b.semver, a.semver));
  for (const version of versionsArray) {
    version.tasks.sort((a, b) =>
      new Date(b.createdAt).getTime() - new Date(a.createdAt).getTime()
    );
  }

  return (
    <div class="mb-20">
      <PackageHeader
        package={data.package}
        downloads={null}
      />

      <PackageNav
        currentTab="Versions"
        params={params as unknown as Params}
        iam={iam}
        versionCount={data.package.versionCount}
        dependencyCount={data.package.dependencyCount}
        dependentCount={data.package.dependentCount}
        latestVersion={data.package.latestVersion}
      />

      <div class="mt-4 md:mt-8">
        <DownloadChart downloads={data.downloads.recentVersions} />
      </div>

      <div class="space-y-3 mt-8">
        {versionsArray.length === 0
          ? (
            <div class="text-tertiary text-center">
              This package has not published any versions yet.
            </div>
          )
          : versionsArray.map((version) => {
            const latestVersion =
              latestVersionInReleaseTrack[version.releaseTrack];
            const isLatestInReleaseTrack = latestVersion
              ? equals(version.semver, latestVersion)
              : false;
            return (
              <Version
                semver={version.semver}
                version={version.version}
                tasks={version.tasks}
                releaseTrack={version.releaseTrack}
                isLatestInReleaseTrack={isLatestInReleaseTrack}
                iam={iam}
              />
            );
          })}
      </div>
    </div>
  );
});

const pluralRule = new Intl.PluralRules("en", { type: "ordinal" });
const pluralSuffixes = new Map([
  ["one", "st"],
  ["two", "nd"],
  ["few", "rd"],
  ["other", "th"],
]);

function ordinalNumber(number: number): string {
  const suffix = pluralSuffixes.get(pluralRule.select(number));
  return `${number}${suffix}`;
}

const statusVerb: Record<PublishingTaskStatus, string> = {
  "pending": "is queued",
  "success": "succeeded",
  "failure": "failed",
  "processed": "is processing",
  "processing": "is processing",
};

function Version({
  semver,
  version,
  tasks,
  releaseTrack,
  isLatestInReleaseTrack,
  iam,
}: {
  semver: SemVer;
  version: PackageVersionWithUser | null;
  tasks: PublishingTask[];
  releaseTrack: string;
  isLatestInReleaseTrack: boolean;
  iam: ScopeIAM;
}) {
  const isPublished = version !== null;
  const isFailed = tasks.length > 0 && tasks[0].status === "failure";
  const isSuccess = tasks.length > 0 &&
    tasks.some((task) => task.status === "success");

  return (
    <div
      class={`relative py-2 px-2 md:py-3 md:px-6 border rounded-lg ${
        (!isPublished && (isFailed || isSuccess)) || version?.yanked
          ? `bg-red-50 dark:bg-red-900/30 border-red-200 dark:border-red-800 ${
            (version?.yanked || (!isPublished && isSuccess))
              ? "hover:bg-red-100 dark:hover:bg-red-800/40 hover:border-red-300 dark:hover:border-red-700"
              : ""
          }`
          : (!isPublished
            ? "bg-blue-50 dark:bg-blue-900/30 border-blue-200 dark:border-blue-700"
            : (isLatestInReleaseTrack
              ? "bg-green-50 dark:bg-green-900/30 hover:bg-green-100 dark:hover:bg-green-800/40 border-green-300 dark:border-green-700 hover:border-green-400 dark:hover:border-green-600"
              : "hover:bg-jsr-gray-100 dark:hover:bg-jsr-gray-900 border-jsr-gray-100 dark:border-jsr-gray-900 hover:border-jsr-gray-300 dark:hover:border-jsr-gray-800"))
      }`}
    >
      <div class="flex items-center justify-between">
        <div class="flex items-center gap-2 md:gap-6">
          <div
            class={`rounded-full size-12 flex items-center justify-center border hover:shadow z-20 select-none font-bold text-xs ${
              (!isPublished && isFailed) || version?.yanked
                ? "bg-red-300 border-red-400 text-red-700"
                : (!isPublished
                  ? isSuccess
                    ? "bg-red-300 border-red-400 text-red-700"
                    : "bg-blue-300 border-blue-400 text-blue-700"
                  : (isLatestInReleaseTrack
                    ? "bg-green-300 border-green-400 text-green-700"
                    : "bg-jsr-gray-200 dark:bg-jsr-gray-900 border-jsr-gray-300 dark:border-jsr-gray-800 text-secondary"))
            }`}
            title={version?.yanked
              ? "Yanked"
              : (isFailed
                ? "Task failed"
                : !isPublished
                ? isSuccess ? "Deleted" : "Publishing..."
                : `Release Track ${releaseTrack}`)}
          >
            {version?.yanked
              ? <TbTrashX class="size-8" />
              : (isFailed
                ? <TbAlertCircle class="size-8 stroke-red-500 stroke-2" />
                : !isPublished
                ? isSuccess ? <TbTrashX class="size-8" /> : "..."
                : releaseTrack)}
          </div>
          <div>
            {isPublished
              ? (
                <a
                  class="font-bold z-10 after:absolute after:inset-0 after:content-empty"
                  href={`/@${version.scope}/${version.package}@${version.version}`}
                >
                  {format(semver)}
                </a>
              )
              : (
                <span class="font-bold z-10 after:absolute after:inset-0 after:content-empty dark:text-gray-200">
                  {format(semver)}
                </span>
              )}
            {isPublished && (
              <div class="text-sm select-none text-tertiary z-0">
                Released {version?.user && (
                  <>
                    {"by "}
                    <a
                      class="link z-20 font-bold"
                      href={`/user/${version.user.id}`}
                    >
                      {version.user.name}
                    </a>
                    {" "}
                  </>
                )}
                {timeAgo(version.createdAt)}
              </div>
            )}
          </div>
        </div>
        {isPublished && iam.canAdmin && (
          <form method="POST" class="z-20">
            <input type="hidden" name="version" value={version.version} />
            <button
              type="submit"
              class="button-danger"
              name="action"
              value={version.yanked ? "unyank" : "yank"}
            >
              {version.yanked ? "Unyank" : "Yank"}
            </button>
          </form>
        )}
        {isPublished && iam.hasSudo && (
          <form method="POST" class="z-20">
            <input type="hidden" name="version" value={version.version} />
            <button
              type="submit"
              class="button-danger"
              name="action"
              value="delete"
            >
              Delete
            </button>
          </form>
        )}
      </div>
      <ul>
        {tasks.map((task, i) => (
          <li class="first:mt-3 mt-1 text-sm flex items-center gap-1 text-tertiary w-full">
            {task.status === "failure"
              ? <TbAlertCircle class="size-4 stroke-red-500 stroke-2" />
              : task.status === "success"
              ? <TbCheck class="size-4 stroke-green-500 stroke-2" />
              : <TbClockHour3 class="size-4 stroke-blue-500 stroke-2" />}
            <span>
              {ordinalNumber(tasks.length - i)} publishing attempt{" "}
              {statusVerb[task.status]} {timeAgo(task.updatedAt)}
            </span>
            <a href={`/status/${task.id}`} class="link justify-self-end z-20">
              Details
            </a>
          </li>
        ))}
      </ul>
    </div>
  );
}

export const handler = define.handlers({
  async GET(ctx) {
    const [res, versionsResp, tasksResp] = await Promise.all([
      packageData(ctx.state, ctx.params.scope, ctx.params.package),
      ctx.state.api.get<PackageVersionWithUser[]>(
        path`/scopes/${ctx.params.scope}/packages/${ctx.params.package}/versions`,
      ),
      ctx.state.api.hasToken()
        ? ctx.state.api.get<PublishingTask[]>(
          path`/scopes/${ctx.params.scope}/packages/${ctx.params.package}/publishing_tasks`,
        )
        : Promise.resolve(null),
    ]);
    if (res === null) throw new HttpError(404, "This package was not found.");

    if (!versionsResp.ok) throw versionsResp; // TODO: handle errors gracefully
    let publishingTasks;
    if (tasksResp) {
      if (!tasksResp.ok) {
        if (tasksResp.code !== "actorNotScopeMember") {
          throw tasksResp; // TODO: handle errors gracefully
        }
      } else {
        publishingTasks = tasksResp.data;
      }
    }

    ctx.state.meta = {
      title: `Versions - @${res.pkg.scope}/${res.pkg.name} - JSR`,
      description: `@${res.pkg.scope}/${res.pkg.name} on JSR${
        res.pkg.description ? `: ${res.pkg.description}` : ""
      }`,
    };
    return {
      data: {
        package: res.pkg,
        versions: versionsResp.data,
        publishingTasks,
        member: res.scopeMember,
        downloads: res.downloads,
      },
    };
  },

  async POST(ctx) {
    const req = ctx.req;
    const {
      scope,
      package: packageName,
    } = ctx.params;
    const { api } = ctx.state;
    const data = await req.formData();

    const action = String(data.get("action"));

    switch (action) {
      case "yank": {
        const version = String(data.get("version"));
        const res = await api.patch(
          path`/scopes/${scope}/packages/${packageName}/versions/${version}`,
          { yanked: true },
        );
        if (!res.ok) throw res;
        return new Response(null, {
          status: 303,
          headers: { Location: `/@${scope}/${packageName}/versions` },
        });
      }
      case "unyank": {
        const version = String(data.get("version"));
        const res = await api.patch(
          path`/scopes/${scope}/packages/${packageName}/versions/${version}`,
          { yanked: false },
        );
        if (!res.ok) throw res;
        return new Response(null, {
          status: 303,
          headers: { Location: `/@${scope}/${packageName}/versions` },
        });
      }
      case "delete": {
        const version = String(data.get("version"));
        const res = await api.delete(
          path`/scopes/${scope}/packages/${packageName}/versions/${version}`,
        );
        if (!res.ok) throw res;
        return new Response(null, {
          status: 303,
          headers: { Location: `/@${scope}/${packageName}/versions` },
        });
      }
      default: {
        throw new Error("Invalid action " + action);
      }
    }
  },
});

export const config: RouteConfig = {
  routeOverride: "/@:scope/:package/versions",
};
