// Copyright 2024 the JSR authors. All rights reserved. MIT license.
import { useSignal, useSignalEffect } from "@preact/signals";
import TbArrowRight from "tb-icons/TbArrowRight";

const SELECT_CLASSES =
  "block w-64 py-1.5 px-2 input-container select text-sm font-normal mt-1";

export default function DiffVersionSelector(
  props: {
    scope: string;
    pkg: string;
    versions: string[];
    oldVersion?: string;
    newVersion?: string;
  },
) {
  return (
    <div class="flex justify-center items-center gap-4">
      <select
        class={SELECT_CLASSES}
        onChange={(e) => {
          location.href = `/@${props.scope}/${props.pkg}/diff/${
            e.currentTarget.value || ""
          }...${props.newVersion || ""}`;
        }}
      >
        <option disabled selected={!props.oldVersion}>select a version</option>
        {props.versions.map((version) => (
          <option value={version} selected={version === props.oldVersion}>
            {version}
          </option>
        ))}
      </select>

      <TbArrowRight class="size-6" />

      <select
        class={SELECT_CLASSES}
        onChange={(e) => {
          location.href = `/@${props.scope}/${props.pkg}/diff/${
            props.oldVersion || ""
          }...${e.currentTarget.value || ""}`;
        }}
      >
        <option disabled selected={!props.newVersion}>select a version</option>
        {props.versions.map((version) => (
          <option value={version} selected={version === props.newVersion}>
            {version}
          </option>
        ))}
      </select>
    </div>
  );
}
