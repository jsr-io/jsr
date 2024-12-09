// Copyright 2024 the JSR authors. All rights reserved. MIT license.
import type { ComponentChildren } from "preact";
import { useCallback, useEffect, useRef } from "preact/hooks";
import { useSignal } from "@preact/signals";
import { instance, type Viz } from "@viz-js/viz";
import { ChevronDown } from "../../../components/icons/ChevronDown.tsx";
import { ChevronLeft } from "../../../components/icons/ChevronLeft.tsx";
import { ChevronRight } from "../../../components/icons/ChevronRight.tsx";
import { ChevronUp } from "../../../components/icons/ChevronUp.tsx";
import { Minus } from "../../../components/icons/Minus.tsx";
import { Plus } from "../../../components/icons/Plus.tsx";
import { Reset } from "../../../components/icons/Reset.tsx";
import { DependencyGraphItem } from "../../../utils/api_types.ts";

export interface DependencyGraphProps {
  dependencies: DependencyGraphItem[];
}

interface DependencyGraphKindGroupedJsr {
  type: "jsr";
  scope: string;
  package: string;
  version: string;
  paths: string[];
}
interface DependencyGraphKindNpm {
  type: "npm";
  package: string;
  version: string;
}
interface DependencyGraphKindRoot {
  type: "root";
  path: string;
}
interface DependencyGraphKindError {
  type: "error";
  error: string;
}

type GroupedDependencyGraphKind =
  | DependencyGraphKindGroupedJsr
  | DependencyGraphKindNpm
  | DependencyGraphKindRoot
  | DependencyGraphKindError;

export interface GroupedDependencyGraphItem {
  dependency: GroupedDependencyGraphKind;
  children: number[];
  size: number | undefined;
  mediaType: string | undefined;
}

interface JsrPackage {
  scope: string;
  package: string;
  version: string;
}

export function groupDependencies(
  items: DependencyGraphItem[],
): GroupedDependencyGraphItem[] {
  const referencedBy = new Map<number, Set<number>>();
  for (let i = 0; i < items.length; i++) {
    for (const child of items[i].children) {
      if (!referencedBy.has(child)) {
        referencedBy.set(child, new Set());
      }
      referencedBy.get(child)!.add(i);
    }
  }

  const jsrGroups = new Map<string, {
    key: JsrPackage;
    paths: { path: string; oldIndex: number }[];
    children: number[];
    size: number | undefined;
    mediaType: string | undefined;
    oldIndices: number[];
  }>();

  for (let i = 0; i < items.length; i++) {
    const item = items[i];
    if (item.dependency.type === "jsr") {
      const groupKey =
        `${item.dependency.scope}/${item.dependency.package}@${item.dependency.version}`;
      const group = jsrGroups.get(groupKey) ?? {
        key: {
          scope: item.dependency.scope,
          package: item.dependency.package,
          version: item.dependency.version,
        },
        paths: [],
        children: [],
        size: undefined,
        mediaType: undefined,
        oldIndices: [],
      };
      group.paths.push({ path: item.dependency.path, oldIndex: i });
      group.children.push(...item.children);
      if (item.size !== undefined) {
        group.size ??= 0;
        group.size += item.size;
      }
      group.oldIndices.push(i);
      jsrGroups.set(groupKey, group);
    }
  }

  const oldIndexToNewIndex = new Map<number, number>();
  const placedJsrGroups = new Set<string>();
  const out: GroupedDependencyGraphItem[] = [];

  for (let i = 0; i < items.length; i++) {
    const item = items[i];
    if (item.dependency.type === "jsr") {
      const groupKey =
        `${item.dependency.scope}/${item.dependency.package}@${item.dependency.version}`;
      const group = jsrGroups.get(groupKey)!;

      if (!placedJsrGroups.has(groupKey)) {
        placedJsrGroups.add(groupKey);

        const groupIndicesSet = new Set(group.oldIndices);
        const filteredPaths = group.paths.filter(({ oldIndex }) => {
          const refs = referencedBy.get(oldIndex)!;

          for (const ref of refs) {
            if (!groupIndicesSet.has(ref)) {
              return true;
            }
          }

          return false; // all references are from within the same jsr package
        }).map((p) => p.path);

        const uniqueChildren = Array.from(new Set(group.children));
        const newIndex = out.length;
        out.push({
          dependency: {
            type: "jsr",
            scope: group.key.scope,
            package: group.key.package,
            version: group.key.version,
            paths: Array.from(new Set(filteredPaths)),
          },
          children: uniqueChildren,
          size: group.size,
          mediaType: group.mediaType,
        });

        for (const oldIdx of group.oldIndices) {
          oldIndexToNewIndex.set(oldIdx, newIndex);
        }
      } else {
        oldIndexToNewIndex.set(
          i,
          oldIndexToNewIndex.get(jsrGroups.get(groupKey)!.oldIndices[0])!,
        );
      }
    } else {
      out.push({
        dependency: item.dependency,
        children: item.children,
        size: item.size,
        mediaType: item.mediaType,
      });
      oldIndexToNewIndex.set(i, out.length - 1);
    }
  }

  for (let index = 0; index < out.length; index++) {
    const newItem = out[index];
    const remappedChildren = newItem.children
      .map((childIdx) => oldIndexToNewIndex.get(childIdx)!)
      .filter((childNewIdx) => childNewIdx !== index);
    newItem.children = Array.from(new Set(remappedChildren));
  }

  return out;
}

function createDigraph(dependencies: DependencyGraphItem[]) {
  const groupedDependencies = groupDependencies(dependencies);

  return `digraph "dependencies" {
  graph [rankdir="LR"]
  node [fontname="Courier", shape="box", style="filled,rounded"]

${
    groupedDependencies.map(({ children, dependency, size }, index) => {
      return [
        `  ${index} ${renderDependency(dependency, size)}`,
        ...children.map((child) => `  ${index} -> ${child}`),
      ].filter(Boolean).join("\n");
    }).join("\n")
  }
}`;
}

function bytesToSize(bytes: number) {
  const sizes = ["B", "KB", "MB", "GB", "TB"];
  if (bytes == 0) return "0 B";
  const i = Math.floor(Math.log(bytes) / Math.log(1024));
  return (bytes / Math.pow(1024, i)).toFixed(0) + " " + sizes[i];
}

function renderDependency(
  dependency: GroupedDependencyGraphKind,
  size?: number,
) {
  let href;
  let content;
  let tooltip;
  let color;
  switch (dependency.type) {
    case "jsr": {
      tooltip =
        `@${dependency.scope}/${dependency.package}@${dependency.version}`;
      href = `/${tooltip}`;
      content = `${tooltip}\n${dependency.paths.join("\n")}\n${
        bytesToSize(size ?? 0)
      }`;
      color = "#faee4a";
      break;
    }
    case "npm": {
      content = tooltip = `${dependency.package}@${dependency.version}`;
      href = `https://www.npmjs.com/package/${dependency.package}`;
      color = "#cb3837";
      break;
    }
    case "root": {
      content = tooltip = dependency.path;
      color = "#67bef9";
      break;
    }
    case "error":
    default:
      content = tooltip = dependency.error;
      break;
  }

  return `[${
    Object
      .entries({ href, tooltip, label: content, color })
      .filter(([_, v]) => v)
      .map(([k, v]) => `${k}="${v}"`)
      .join(", ")
  }]`;
}

function useDigraph(dependencies: DependencyGraphItem[]) {
  const controls = useSignal({ pan: { x: 0, y: 0 }, zoom: 1 });
  const defaults = useSignal({ pan: { x: 0, y: 0 }, zoom: 1 });
  const ref = useRef<HTMLDivElement>(null);
  const svg = useRef<SVGSVGElement | null>(null);
  const viz = useSignal<Viz | undefined>(undefined);

  const center = useCallback(() => {
    if (svg.current && ref.current) {
      const { width: sWidth, height: sHeight } = svg.current
        .getBoundingClientRect();
      const { width: rWidth, height: rHeight } = ref.current
        .getBoundingClientRect();

      defaults.value.pan.x = (rWidth - sWidth) / 2;
      defaults.value.pan.y = (rHeight - sHeight) / 2;
      defaults.value.zoom = Math.min(rWidth / sWidth, rHeight / sHeight);
      controls.value = structuredClone(defaults.value);
      svg.current.style.transform =
        `translate(${controls.value.pan.x}px, ${controls.value.pan.y}px) scale(${controls.value.zoom})`;
    }
  }, []);

  const pan = useCallback((x: number, y: number) => {
    controls.value.pan.x += x;
    controls.value.pan.y += y;
    if (svg.current) {
      svg.current.style.transform =
        `translate(${controls.value.pan.x}px, ${controls.value.pan.y}px) scale(${controls.value.zoom})`;
    }
  }, []);

  const zoom = useCallback((zoom: number) => {
    controls.value.zoom = Math.max(
      0.1,
      Math.min(controls.value.zoom + zoom, 2.5),
    );

    if (svg.current) {
      svg.current.style.transform =
        `translate(${controls.value.pan.x}px, ${controls.value.pan.y}px) scale(${controls.value.zoom})`;
    }
  }, []);

  const reset = useCallback(() => {
    controls.value = structuredClone(defaults.value);
    if (svg.current) {
      svg.current.style.transform =
        `translate(${controls.value.pan.x}px, ${controls.value.pan.y}px) scale(${controls.value.zoom})`;
    }
  }, []);

  useEffect(() => {
    (async () => {
      viz.value = await instance();

      if (ref.current && viz.value) {
        const digraph = createDigraph(dependencies);

        svg.current = viz.value.renderSVGElement(digraph);
        ref.current.prepend(svg.current);

        center();
      }
    })();
  }, [dependencies]);

  return { pan, zoom, reset, ref };
}

interface GraphControlButtonProps {
  children: ComponentChildren;
  class: string;
  onClick: () => void;
  title: string;
}

function GraphControlButton(props: GraphControlButtonProps) {
  return (
    <button
      aria-label={props.title}
      class={`${props.class} bg-white text-jsr-gray-700 p-1.5 ring-1 ring-jsr-gray-700 rounded-full sm:rounded hover:bg-jsr-gray-100/30"`}
      onClick={props.onClick}
      title={props.title}
    >
      {props.children}
    </button>
  );
}

export function DependencyGraph(props: DependencyGraphProps) {
  const { pan, zoom, reset, ref } = useDigraph(props.dependencies);
  const dragActive = useSignal(false);

  function enableDrag() {
    dragActive.value = true;
  }
  function disableDrag() {
    dragActive.value = false;
  }

  function onMouseMove(event: MouseEvent) {
    if (dragActive.value) {
      pan(event.movementX, event.movementY);
    }
  }

  function wheelZoom(event: WheelEvent) {
    event.preventDefault();
    // TODO: zoom on pointer
    zoom(event.deltaY / 250);
  }

  return (
    <div
      class="-mx-4 md:mx-0 ring-1 ring-jsr-cyan-100 sm:rounded overflow-hidden relative h-[90vh]"
      onMouseDown={enableDrag}
      onMouseMove={onMouseMove}
      onMouseUp={disableDrag}
      onMouseLeave={disableDrag}
      onWheel={wheelZoom}
      ref={ref}
    >
      <div class="absolute gap-1 grid grid-cols-3 bottom-4 right-4">
        {/* zoom */}
        <GraphControlButton
          class="col-start-3 col-end-3 row-start-1 row-end-1"
          onClick={() => zoom(0.1)}
          title="Zoom in"
        >
          <Plus />
        </GraphControlButton>
        <GraphControlButton
          class="col-start-3 col-end-3 row-start-3 row-end-3"
          onClick={() => zoom(-0.1)}
          title="Zoom out"
        >
          <Minus />
        </GraphControlButton>

        {/* pan */}
        <GraphControlButton
          class="col-start-2 col-end-2 row-start-1 row-end-1"
          onClick={() => pan(0, 100)}
          title="Pan up"
        >
          <ChevronUp />
        </GraphControlButton>
        <GraphControlButton
          class="col-start-1 col-end-1 row-start-2 row-end-2"
          onClick={() => pan(100, 0)}
          title="Pan left"
        >
          <ChevronLeft />
        </GraphControlButton>
        <GraphControlButton
          class="col-start-3 col-end-3 row-start-2 row-end-2"
          onClick={() => pan(-100, 0)}
          title="Pan right"
        >
          <ChevronRight />
        </GraphControlButton>
        <GraphControlButton
          class="col-start-2 col-end-2 row-start-3 row-end-3"
          onClick={() => pan(0, -100)}
          title="Pan down"
        >
          <ChevronDown />
        </GraphControlButton>

        {/* reset */}
        <GraphControlButton
          class="col-start-2 col-end-2 row-start-2 row-end-2"
          onClick={reset}
          title="Reset view"
        >
          <Reset />
        </GraphControlButton>
      </div>
    </div>
  );
}
