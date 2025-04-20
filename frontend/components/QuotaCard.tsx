// Copyright 2024 the JSR authors. All rights reserved. MIT license.
import { ComponentChildren } from "preact";

export function QuotaCard(
  props: {
    title: ComponentChildren;
    description: ComponentChildren;
    limit: number;
    usage: number;
  },
) {
  return (
    <div class="border-1.5 border-jsr-gray-200 dark:border-jsr-gray-700 rounded-md px-4 py-5 flex flex-col justify-between dark:bg-jsr-gray-900">
      <div>
        <p class="font-semibold text-primary">{props.title}</p>
        <p class="text-secondary text-sm">{props.description}</p>
      </div>
      <QuotaUsage limit={props.limit} usage={props.usage} />
    </div>
  );
}

function QuotaUsage(props: { limit: number; usage: number }) {
  const percent = props.usage / props.limit;

  let bgColor = "bg-jsr-yellow-400";
  const ringColor = percent >= 1 ? "ring-red-700" : "ring-jsr-yellow-700";

  if (percent >= 1) {
    bgColor = "bg-red-500";
  } else if (percent > 0.9) {
    bgColor = "bg-orange-400";
  } else if (percent > 0.8) {
    bgColor = "bg-jsr-yellow-500";
  }

  return (
    <div class="mt-4 flex items-center gap-2">
      <div
        class={`overflow-hidden h-3 w-full rounded bg-jsr-yellow-50 dark:bg-jsr-gray-900 ring-1 ${ringColor}`}
      >
        <div
          style={{ width: `${percent * 100}%` }}
          // We deduplicate ring classes here to avoid whitespace between "bar" and "ring"
          class={`h-full ${bgColor} ring-1 ${ringColor}`}
        >
        </div>
      </div>
      <div class="text-xs text-secondary">{props.usage}/{props.limit}</div>
    </div>
  );
}
