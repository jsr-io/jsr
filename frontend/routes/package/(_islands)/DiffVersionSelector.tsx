// Copyright 2024 the JSR authors. All rights reserved. MIT license.
import TbArrowRight from "tb-icons/TbArrowRight";
import TbEyeFilled from "tb-icons/TbEyeFilled";
import TbEyeOff from "tb-icons/TbEyeOff";

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
    docsRequest: string;
  },
) {
  const full = props.url.searchParams.has("full");
  const strippedUrl = new URL(props.url);
  strippedUrl.searchParams.delete("full");

  return (
    <div class="flex justify-center items-center gap-3.5">
      <select
        class={SELECT_CLASSES}
        onChange={(e) => {
          location.href = `/@${props.scope}/${props.pkg}/diff/${
            e.currentTarget.value || ""
          }...${props.newVersion || ""}${props.docsRequest}`;
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
        class="p-1 -ml-1 rounded-md hover:bg-jsr-cyan-100 dark:hover:bg-jsr-cyan-950"
        href={`/@${props.scope}/${props.pkg}/diff/${props.newVersion || ""}...${
          props.oldVersion || ""
        }${props.docsRequest}`}
      >
        <TbArrowRight class="size-6" />
      </a>

      <select
        class={SELECT_CLASSES}
        onChange={(e) => {
          location.href = `/@${props.scope}/${props.pkg}/diff/${
            props.oldVersion || ""
          }...${e.currentTarget.value || ""}${props.docsRequest}`;
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
        class="p-1 -ml-1 rounded-md hover:bg-jsr-cyan-100 dark:hover:bg-jsr-cyan-950"
        href={strippedUrl.pathname + (full ? "" : "?full")}
        title={full ? "Only show changed items" : "Show all items"}
      >
        {full ? <TbEyeOff class="size-6" /> : <TbEyeFilled class="size-6" />}
      </a>
    </div>
  );
}
