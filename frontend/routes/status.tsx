// Copyright 2024 the JSR authors. All rights reserved. MIT license.
import { Handlers, PageProps, RouteConfig } from "$fresh/server.ts";
import { Head } from "$fresh/runtime.ts";
import { State } from "../util.ts";
import type {
  Package,
  PublishingTask,
  PublishingTaskStatus,
  ScopeMember,
} from "../utils/api_types.ts";
import { path } from "../utils/api.ts";
import { packageData } from "../utils/data.ts";
import { PackageHeader } from "./package/(_components)/PackageHeader.tsx";
import { PackageNav } from "./package/(_components)/PackageNav.tsx";
import twas from "$twas";
import PublishingTaskRequeue from "../islands/PublishingTaskRequeue.tsx";
import { Pending } from "../components/icons/Pending.tsx";
import { Check } from "../components/icons/Check.tsx";
import { ErrorIcon } from "../components/icons/Error.tsx";
import { scopeIAM } from "../utils/iam.ts";

interface Data {
  package: Package;
  publishingTask: PublishingTask;
  member: ScopeMember | null;
}

export default function PackageListPage(
  { data, state }: PageProps<Data, State>,
) {
  const iam = scopeIAM(state, data.member);

  return (
    <div class="mb-24 space-y-16">
      <Head>
        <title>
          Publishing Task {data.publishingTask.id} - JSR
        </title>
      </Head>
      <div>
        <PackageHeader package={data.package} />

        <PackageNav
          currentTab="Versions"
          versionCount={data.package.versionCount}
          iam={iam}
          params={{ scope: data.package.scope, package: data.package.name }}
          latestVersion={data.package.latestVersion}
        />

        <div class="mt-8 space-y-2">
          <h2 class="text-xl font-sans font-bold">
            Publishing Status
          </h2>
          <div>
            <p>
              <span class="font-semibold">Version:</span>{" "}
              {data.publishingTask.packageVersion}
            </p>
            <p>
              <span class="font-semibold">Task ID:</span>{" "}
              <span class="font-mono">{data.publishingTask.id}</span>
            </p>
            <p>
              <span class="font-semibold">Created:</span>{" "}
              {twas(new Date(data.publishingTask.createdAt))}
            </p>
            {data.publishingTask.userId && (
              <p>
                <span class="font-semibold">Submitter:</span>{" "}
                <a
                  class="link italic"
                  href={`/user/${data.publishingTask.userId}`}
                >
                  View user
                </a>
              </p>
            )}
            <p class="flex items-center gap-1">
              <span class="font-semibold">Status:</span>{" "}
              {StatusToIcon(data.publishingTask.status)}{" "}
              {data.publishingTask.status}
            </p>
          </div>

          {data.publishingTask.error && (
            <div class="bg-red-100 rounded border-2 border-red-200 py-1.5 px-3 flex justify-between gap-3 dark:bg-red-500 dark:border-red-600 dark:text-white">
              <div class="space-y-1.5">
                <div class="font-bold text-xl">
                  {data.publishingTask.error.code}
                </div>
                <div>
                  {data.publishingTask.error.message}
                </div>
              </div>
            </div>
          )}

          {data.publishingTask.status === "success" && (
            <p>
              <a
                href={`/@${data.publishingTask.packageScope}/${data.publishingTask.packageName}@${data.publishingTask.packageVersion}`}
                class="link"
              >
                View published version
              </a>
            </p>
          )}

          {iam.isStaff && (
            <PublishingTaskRequeue publishingTask={data.publishingTask} />
          )}
        </div>
      </div>
    </div>
  );
}

export function StatusToIcon(status: PublishingTaskStatus) {
  switch (status) {
    case "pending":
    case "processing":
    case "processed":
      return <Pending class="size-6 stroke-blue-500 stroke-2" />;
    case "success":
      return <Check class="size-6 stroke-green-500 stroke-2" />;
    case "failure":
      return <ErrorIcon class="size-6 stroke-red-500 stroke-2" />;
  }
}

export const handler: Handlers<Data, State> = {
  async GET(_req, ctx) {
    const publishingTaskResp = await ctx.state.api.get<PublishingTask>(
      path`/publishing_tasks/${ctx.params.publishingTask}`,
    );
    if (!publishingTaskResp.ok) throw publishingTaskResp; // gracefully handle this

    const res = await packageData(
      ctx.state,
      publishingTaskResp.data.packageScope,
      publishingTaskResp.data.packageName,
    );
    if (res === null) return ctx.renderNotFound();

    return ctx.render({
      package: res.pkg,
      member: res.scopeMember,
      publishingTask: publishingTaskResp.data,
    });
  },
};

export const config: RouteConfig = {
  routeOverride: "/status/:publishingTask",
};
