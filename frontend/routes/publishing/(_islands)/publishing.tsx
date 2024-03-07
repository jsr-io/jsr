import { computed, Signal, useSignalEffect } from "@preact/signals";
import { useEffect } from "preact/hooks";
import { api, path } from "../../../utils/api.ts";
import {
  PublishingTask,
  PublishingTaskStatus,
} from "../../../utils/api_types.ts";
import { ErrorIcon } from "../../../components/icons/Error.tsx";
import { Check } from "../../../components/icons/Check.tsx";
import { Pending } from "../../../components/icons/Pending.tsx";

export interface VersionPublishStatus {
  loading: boolean;
  task?: PublishingTask;
}

const statusVerb: Record<PublishingTaskStatus, string> = {
  "pending": "is queued",
  "success": "succeeded",
  "failure": "failed",
  "processed": "is processing",
  "processing": "is processing",
};

export function PackagePublishStatus(props: {
  name: string;
  version: string;
  date: string;
  status: Signal<VersionPublishStatus>;
}) {
  useEffect(() => {
    const [scope, name] = props.name.slice(1).split("/");
    const version = props.version;
    const date = new Date(props.date);
    let cancel = false;

    (async () => {
      while (true) {
        if (cancel) break;
        const task = await poll(scope, name, version, date);
        props.status.value = {
          loading: false,
          task,
        };
        if (task?.status === "success" || task?.status === "failure") break;
        await new Promise((resolve) => setTimeout(resolve, 4000));
      }
    })().catch((e) => console.error("polling error", scope, name, version, e));

    (() => cancel = true);
  }, [props.name, props.version, props.date]);

  const { loading, task } = props.status.value;

  if (loading) {
    return <p class="italic text-gray-600 max-w-2xl">...</p>;
  }

  if (!task) {
    return (
      <p class="italic text-gray-600 max-w-2xl">
        Publishing has not started yet...
      </p>
    );
  }

  return (
    <>
      <p
        class={`flex items-center gap-1 max-w-2xl ${
          task.status === "failure"
            ? "text-red-700"
            : task.status === "success"
            ? "text-green-700"
            : "text-blue-700"
        }`}
      >
        {task.status === "failure"
          ? (
            <ErrorIcon class="size-5 stroke-red-700 bg-red-200 rounded-full p-0.5 stroke-2" />
          )
          : task.status === "success"
          ? (
            <Check class="size-5 stroke-green-700 stroke-2 bg-green-200 rounded-full p-0.5" />
          )
          : (
            <Pending class="size-5 stroke-blue-700 bg-blue-200 rounded-full p-0.5 animate-pulse stroke-2" />
          )}
        Publish {statusVerb[task.status]}
      </p>
      {task.error && (
        <p class="text-red-700 max-w-3xl ml-6 text-sm">
          <span class="font-mono font-semibold">{task.error.code}</span>:{" "}
          {task.error.message}
        </p>
      )}
    </>
  );
}

async function poll(
  scope: string,
  name: string,
  version: string,
  date: Date,
): Promise<PublishingTask | undefined> {
  const resp = await api.get<PublishingTask[]>(
    path`/scopes/${scope}/packages/${name}/publishing_tasks`,
  );
  if (!resp.ok) {
    console.error(scope, name, resp);
    return undefined;
  }
  for (const task of resp.data) {
    if (task.packageVersion !== version) continue;
    if (task.status === "success") return task;
    if (new Date(task.createdAt).getTime() > date.getTime()) return task;
  }
  return undefined;
}

export function OverallStatus(
  props: {
    packages: {
      name: string;
      version: string;
      status: Signal<VersionPublishStatus>;
    }[];
  },
) {
  const anyLoading = computed(() =>
    props.packages.find((p) => p.status.value.loading) !== undefined
  );

  const anyFailed = computed(() =>
    props.packages.find((p) => p.status.value.task?.status === "failure") !==
      undefined
  );
  const success = computed(() =>
    props.packages.every((p) => p.status.value.task?.status === "success")
  );

  useSignalEffect(() => {
    if (success.value && props.packages.length === 1) {
      setTimeout(() => {
        location.href = `/${props.packages[0].name}@${
          props.packages[0].version
        }`;
      }, 200);
    }
  });

  return (
    <div
      class={`border-2 py-1 px-4 mt-4 ${
        anyFailed.value
          ? "bg-red-50 border-red-200 text-red-700"
          : success.value
          ? "bg-green-50 border-green-200 text-green-700"
          : "bg-jsr-cyan-50 border-jsr-cyan-200 text-jsr-cyan-700"
      }`}
    >
      {anyFailed.value
        ? "A package has failed publishing. The publishing task in the terminal may be able to provide more information about the issue."
        : success.value
        ? "All packages were published successfully."
        : anyLoading.value
        ? <span class="italic">Loading publishing status...</span>
        : "Waiting for some packages to complete publishing."}
    </div>
  );
}

export function PackageLink(props: { status: Signal<VersionPublishStatus> }) {
  const task = props.status.value.task;
  if (task?.status === "success") {
    return (
      <a
        href={`/@${task.packageScope}/${task.packageName}@${task.packageVersion}`}
        class="link text-base font-medium ml-4"
      >
        View package version
      </a>
    );
  }

  return null;
}
