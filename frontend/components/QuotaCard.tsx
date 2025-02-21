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
    <div class="border-1.5 border-jsr-gray-500/20 rounded-md px-4 py-5 flex flex-col justify-between">
      <div>
        <p class="font-semibold text-foreground-secondary">{props.title}</p>
        <p class="text-foreground-secondary text-sm">{props.description}</p>
      </div>
      <QuotaUsage limit={props.limit} usage={props.usage} />
    </div>
  );
}

function QuotaUsage(props: { limit: number; usage: number }) {
  let color = "bg-jsr-yellow-400";
  const percent = props.usage / props.limit;
  if (percent >= 1) {
    color = "bg-red-500";
  } else if (percent > 0.9) {
    color = "bg-orange-400";
  } else if (percent > 0.8) {
    color = "bg-jsr-yellow-500";
  }

  return (
    <div class="mt-4 flex items-center gap-2">
      <div class="overflow-hidden h-3 w-full rounded bg-jsr-yellow-500/20
       ring-1 ring-jsr-yellow-500">
        <div
          style={{ width: `${percent * 100}%` }}
          class={`h-full ${color}`}
        >
        </div>
      </div>
      <div class="text-xs text-foreground-secondary">
        {props.usage}/{props.limit}
      </div>
    </div>
  );
}
