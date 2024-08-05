// Copyright 2024 the JSR authors. All rights reserved. MIT license.
import { Handlers, PageProps, RouteConfig } from "$fresh/server.ts";
import type {
  Package,
  PackageVersionWithUser,
  PublishingTask,
  PublishingTaskStatus,
  ScopeMember,
} from "../../utils/api_types.ts";
import { State } from "../../util.ts";
import { compare, eq, format, lt, parse, SemVer } from "$std/semver/mod.ts";
import twas from "$twas";
import IconTrashX from "$tabler_icons/trash-x.tsx";
import { packageData } from "../../utils/data.ts";
import { PackageHeader } from "./(_components)/PackageHeader.tsx";
import { PackageNav, Params } from "./(_components)/PackageNav.tsx";
import { path } from "../../utils/api.ts";
import { Head } from "$fresh/runtime.ts";
import { ErrorIcon } from "../../components/icons/Error.tsx";
import { Check } from "../../components/icons/Check.tsx";
import { Pending } from "../../components/icons/Pending.tsx";
import { ScopeIAM, scopeIAM } from "../../utils/iam.ts";

interface Data {
  package: Package;
  versions: PackageVersionWithUser[];
  publishingTasks?: PublishingTask[];
  member: ScopeMember | null;
}

export default function Versions({
  data,
  params,
  state,
}: PageProps<Data, State>) {
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
      semver.prerelease.length === 0 &&
      (latestVersionInReleaseTrack[releaseTrack] === undefined ||
        lt(latestVersionInReleaseTrack[releaseTrack], semver))
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
      <Head>
        <title>
          Versions - @{params.scope}/{params.package} - JSR
        </title>
        <meta
          name="description"
          content={`@${params.scope}/${params.package} on JSR${
            data.package.description ? `: ${data.package.description}` : ""
          }`}
        />
      </Head>

      <PackageHeader package={data.package} />

      <PackageNav
        currentTab="Versions"
        params={params as unknown as Params}
        iam={iam}
        versionCount={data.package.versionCount}
        latestVersion={data.package.latestVersion}
      />

      <div class="space-y-3 mt-8">
        {versionsArray.length === 0
          ? (
            <div class="text-jsr-gray-500 text-center">
              This package has not published any versions yet.
            </div>
          )
          : versionsArray.map((version) => {
            const latestVersion =
              latestVersionInReleaseTrack[version.releaseTrack];
            const isLatestInReleaseTrack = latestVersion
              ? eq(version.semver, latestVersion)
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
}

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

  return (
    <div
      class={`relative py-2 px-2 md:py-3 md:px-6 border rounded-lg ${
        (!isPublished && isFailed) || version?.yanked
          ? `bg-red-50 border-red-200 ${
            version?.yanked ? "hover:bg-red-100 hover:border-red-300" : ""
          }`
          : (!isPublished
            ? "bg-blue-50 border-blue-200"
            : (isLatestInReleaseTrack
              ? "bg-green-50 hover:bg-green-100 border-green-300 hover:border-green-400"
              : "hover:bg-jsr-gray-100 border-jsr-gray-100 hover:border-jsr-gray-300"))
      }`}
    >
      <div class="flex items-center justify-between">
        <div class="flex items-center gap-2 md:gap-6">
          <div
            class={`rounded-full size-12 flex items-center justify-center border hover:shadow z-20 select-none font-bold text-xs ${
              (!isPublished && isFailed) || version?.yanked
                ? "bg-red-300 border-red-400 text-red-700"
                : (!isPublished
                  ? "bg-blue-300 border-blue-400 text-blue-700"
                  : (isLatestInReleaseTrack
                    ? "bg-green-300 border-green-400 text-green-700"
                    : "bg-jsr-gray-200 border-jsr-gray-300 text-jsr-gray-600"))
            }`}
            title={version?.yanked
              ? "Yanked"
              : (isFailed
                ? "Task failed"
                : !isPublished
                ? "Publishing..."
                : `Release Track ${releaseTrack}`)}
          >
            {version?.yanked
              ? <IconTrashX class="size-8" />
              : (isFailed
                ? <ErrorIcon class="size-8 stroke-red-500 stroke-2" />
                : !isPublished
                ? "..."
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
                <span class="font-bold z-10 after:absolute after:inset-0 after:content-empty">
                  {format(semver)}
                </span>
              )}
            {isPublished && (
              <div class="text-sm select-none text-jsr-gray-500 z-0">
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
                {twas(new Date(version.createdAt))}
              </div>
            )}
          </div>
        </div>
        {isPublished && iam.canAdmin && (
          <form method="POST" class="z-20">
            <input type="hidden" name="version" value={version.version} />
            <button
              class="button-danger"
              name="action"
              value={version.yanked ? "unyank" : "yank"}
            >
              {version.yanked ? "Unyank" : "Yank"}
            </button>
          </form>
        )}
      </div>
      <ul>
        {tasks.map((task, i) => (
          <li class="first:mt-3 mt-1 text-sm flex items-center gap-1 text-jsr-gray-500 w-full">
            {task.status === "failure"
              ? <ErrorIcon class="size-3 stroke-red-500 stroke-2" />
              : task.status === "success"
              ? <Check class="size-3 stroke-green-500 stroke-2" />
              : <Pending class="size-3 stroke-blue-500 stroke-2" />}
            <span>
              {ordinalNumber(tasks.length - i)} publishing attempt{" "}
              {statusVerb[task.status]} {twas(new Date(task.updatedAt))}
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

export const handler: Handlers<Data, State> = {
  async GET(_, ctx) {
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
    if (res === null) return ctx.renderNotFound();
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

    return ctx.render({
      package: res.pkg,
      versions: versionsResp.data,
      publishingTasks,
      member: res.scopeMember,
    });
  },

  async POST(req, ctx) {
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
      default: {
        throw new Error("Invalid action " + action);
      }
    }
  },
};

export const config: RouteConfig = {
  routeOverride: "/@:scope/:package/versions",
};
