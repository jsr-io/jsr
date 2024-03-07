// Copyright 2024 the JSR authors. All rights reserved. MIT license.
import { computed, Signal } from "@preact/signals";
import { VersionPublishStatus } from "./publishing.tsx";

export function OverallStatus(
  props: { packages: { status: Signal<VersionPublishStatus> }[] },
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
        ? "A package has failed publishing. Your terminal may provide more information about the issue."
        : success.value
        ? "All packages were published successfully."
        : anyLoading.value
        ? <span class="italic">Loading publishing status...</span>
        : "Waiting for some packages to start or complete publishing."}
    </div>
  );
}
