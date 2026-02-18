// Copyright 2024 the JSR authors. All rights reserved. MIT license.
import TbArrowRight from "tb-icons/TbArrowRight";
import TbLayoutBottombarCollapseFilled from "tb-icons/TbLayoutBottombarCollapseFilled";
import TbLayoutBottombarExpandFilled from "tb-icons/TbLayoutBottombarExpandFilled";

const SELECT_CLASSES =
  "block w-64 py-1.5 px-2 input-container select text-sm font-normal";

export default function DiffVersionSelector(
  props: {
    scope: string;
    pkg: string;
    versions: string[];
    oldVersion?: string;
    newVersion?: string;
    url: URL;
  },
) {
  const full = props.url.searchParams.has("full");
  const strippedUrl = new URL(props.url);
  strippedUrl.searchParams.delete("full");

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

      <a
        href={`/@${props.scope}/${props.pkg}/diff/${props.newVersion || ""}...${
          props.oldVersion || ""
        }`}
      >
        <TbArrowRight class="size-6" />
      </a>

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

      <a
        href={strippedUrl.pathname + (full ? "" : "?full")}
        title={full ? "Only show changed items" : "Show all items"}
      >
        {full
          ? <TbLayoutBottombarExpandFilled class="size-6" />
          : <TbLayoutBottombarCollapseFilled class="size-6" />}
      </a>
    </div>
  );
}
