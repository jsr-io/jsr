// Copyright 2024 the JSR authors. All rights reserved. MIT license.
import type { ComponentChildren } from "preact";
import { useCallback, useEffect, useRef } from "preact/hooks";
import { useSignal } from "@preact/signals";
import { instance, type Viz } from "@viz-js/viz";
import {
  TbChevronDown,
  TbChevronLeft,
  TbChevronRight,
  TbChevronUp,
  TbMinus,
  TbPlus,
  TbRefresh,
} from "tb-icons";

import type {
  DependencyGraphItem,
  DependencyGraphKindError,
  DependencyGraphKindNpm,
  DependencyGraphKindRoot,
} from "../../../utils/api_types.ts";
import { format as formatBytes } from "@std/fmt/bytes";

export interface DependencyGraphProps {
  dependencies: DependencyGraphItem[];
}

interface DependencyGraphKindGroupedJsr {
  type: "jsr";
  scope: string;
  package: string;
  version: string;
  entrypoints: string[];
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

function groupDependencies(
  items: DependencyGraphItem[],
): GroupedDependencyGraphItem[] {
  const referencedBy = new Map<number, Set<number>>();
  for (const item of items) {
    for (const childId of item.children) {
      if (!referencedBy.has(childId)) {
        referencedBy.set(childId, new Set());
      }
      referencedBy.get(childId)!.add(item.id);
    }
  }

  const jsrGroups = new Map<string, {
    key: JsrPackage;
    entrypoints: {
      entrypoint: string;
      isEntrypoint: boolean;
      oldId: number;
    }[];
    children: number[];
    size: number | undefined;
    mediaType: string | undefined;
    oldIds: number[];
  }>();

  for (const item of items) {
    if (item.dependency.type === "jsr") {
      const groupKey =
        `${item.dependency.scope}/${item.dependency.package}@${item.dependency.version}`;
      const group = jsrGroups.get(groupKey) ?? {
        key: {
          scope: item.dependency.scope,
          package: item.dependency.package,
          version: item.dependency.version,
        },
        entrypoints: [],
        children: [],
        size: undefined,
        mediaType: undefined,
        oldIds: [],
      };
      group.entrypoints.push({
        entrypoint: item.dependency.entrypoint.value,
        isEntrypoint: item.dependency.entrypoint.type == "entrypoint",
        oldId: item.id,
      });
      group.children.push(...item.children);
      if (item.size !== undefined) {
        group.size ??= 0;
        group.size += item.size;
      }
      group.oldIds.push(item.id);
      jsrGroups.set(groupKey, group);
    }
  }

  const idToIndex = new Map<number, number>();
  const placedJsrGroups = new Set<string>();
  const out: GroupedDependencyGraphItem[] = [];

  for (const item of items) {
    if (item.dependency.type === "jsr") {
      const groupKey =
        `${item.dependency.scope}/${item.dependency.package}@${item.dependency.version}`;
      const group = jsrGroups.get(groupKey)!;

      if (!placedJsrGroups.has(groupKey)) {
        placedJsrGroups.add(groupKey);

        const groupIds = new Set(group.oldIds);
        const filteredEntrypoints = group.entrypoints.filter(({ oldId }) => {
          const refs = referencedBy.get(oldId)!;
          for (const ref of refs) {
            if (!groupIds.has(ref)) {
              return true;
            }
          }

          return false; // all references are from within the same jsr package
        }).map((p) => {
          if (!p.isEntrypoint) {
            throw new Error("unreachable");
          }
          return p.entrypoint;
        });

        const uniqueChildren = Array.from(new Set(group.children));
        const newIndex = out.length;
        out.push({
          dependency: {
            type: "jsr",
            scope: group.key.scope,
            package: group.key.package,
            version: group.key.version,
            entrypoints: Array.from(new Set(filteredEntrypoints)),
          },
          children: uniqueChildren,
          size: group.size,
          mediaType: group.mediaType,
        });

        for (const oldId of group.oldIds) {
          idToIndex.set(oldId, newIndex);
        }
      } else {
        idToIndex.set(
          item.id,
          idToIndex.get(jsrGroups.get(groupKey)!.oldIds[0])!,
        );
      }
    } else {
      out.push({
        dependency: item.dependency,
        children: item.children,
        size: item.size,
        mediaType: item.mediaType,
      });
      idToIndex.set(item.id, out.length - 1);
    }
  }

  for (let index = 0; index < out.length; index++) {
    const newItem = out[index];
    const remappedChildren = newItem.children
      .map((oldId) => idToIndex.get(oldId)!)
      .filter((childNewIdx) => childNewIdx !== index);
    newItem.children = Array.from(new Set(remappedChildren));
  }

  return out;
}

function createDigraph(dependencies: DependencyGraphItem[]) {
  const groupedDependencies = groupDependencies(dependencies);

  const nodesWithNoParent = new Set(
    Object.keys(groupedDependencies).map(Number),
  );

  const depsGraph = groupedDependencies.map(
    ({ children, dependency, size }, index) => {
      return [
        `  ${index} ${renderDependency(dependency, size)}`,
        ...children.map((child) => {
          nodesWithNoParent.delete(child);
          return `  ${index} -> ${child}`;
        }),
      ].filter(Boolean).join("\n");
    },
  ).join("\n");

  return `digraph "dependencies" {
  graph [rankdir="LR", concentrate=true]
  node [fontname="Courier", shape="box", style="filled,rounded-sm"]

  {
    rank=same
    ${Array.from(nodesWithNoParent).join("; ")}
  }

  ${depsGraph}
}`;
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
      content = `${tooltip}\n${
        dependency.entrypoints.map((entrypoint) => {
          if (entrypoint == ".") {
            return "<i>default entrypoint</i>";
          } else {
            return entrypoint;
          }
        }).join("\n")
      }\n${formatBytes(size ?? 0, { maximumFractionDigits: 0 }).toUpperCase()}`;
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

  const renderAttributeValue = (content?: string) => {
    if (!content) return content;

    const hasHTMLTag = /<i>(.+?)<\/i>/.test(content);
    if (hasHTMLTag) {
      const htmlContent = content.replace(/\n/g, "<br/>");
      return `<${htmlContent}>`;
    }
    return `"${content}"`;
  };

  return `[${
    Object
      .entries({ href, tooltip, label: content, color })
      .filter(([_, v]) => v)
      .map(([k, v]) => `${k}=${renderAttributeValue(v)}`)
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

        svg.current = viz.value.renderSVGElement(digraph, {
          engine: "dot",
        });
        svg.current.id = "vizgraph";
        ref.current.prepend(svg.current);

        center();
      }
    })();
  }, [dependencies]);

  return { pan, zoom, reset, svg, ref };
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
      type="button"
      aria-label={props.title}
      class={`${props.class} bg-white dark:bg-jsr-gray-900 text-jsr-gray-700 dark:text-white p-1.5 ring-1 ring-jsr-gray-700 dark:ring-white rounded-full sm:rounded-sm hover:bg-jsr-gray-100/30 dark:hover:bg-jsr-gray-800/50`}
      onClick={props.onClick}
      title={props.title}
    >
      {props.children}
    </button>
  );
}

const DRAG_THRESHOLD = 5;

export function DependencyGraph(props: DependencyGraphProps) {
  const { pan, zoom, reset, svg, ref } = useDigraph(props.dependencies);
  const dragActive = useSignal(false);
  const dragStart = useSignal({ x: 0, y: 0 });

  function enableDrag(event: MouseEvent) {
    dragStart.value = { x: event.clientX, y: event.clientY };
  }

  function disableDrag() {
    dragActive.value = false;
    dragStart.value = { x: 0, y: 0 };
    svg.current?.querySelectorAll("a").forEach((link) => {
      link.style.pointerEvents = "auto";
    });
  }

  function onMouseMove(event: MouseEvent) {
    if (!dragActive.value && (dragStart.value.x || dragStart.value.y)) {
      const dx = Math.abs(event.clientX - dragStart.value.x);
      const dy = Math.abs(event.clientY - dragStart.value.y);
      if (dx > DRAG_THRESHOLD || dy > DRAG_THRESHOLD) {
        dragActive.value = true;
        svg.current?.querySelectorAll("a").forEach((link) => {
          link.style.pointerEvents = "none";
        });
      }
    }
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
      class="-mx-4 md:mx-0 ring-1 ring-jsr-cyan-100 dark:ring-jsr-cyan-900 sm:rounded-sm overflow-hidden relative h-[90vh]"
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
          <TbPlus />
        </GraphControlButton>
        <GraphControlButton
          class="col-start-3 col-end-3 row-start-3 row-end-3"
          onClick={() => zoom(-0.1)}
          title="Zoom out"
        >
          <TbMinus />
        </GraphControlButton>

        {/* pan */}
        <GraphControlButton
          class="col-start-2 col-end-2 row-start-1 row-end-1"
          onClick={() => pan(0, 100)}
          title="Pan up"
        >
          <TbChevronUp />
        </GraphControlButton>
        <GraphControlButton
          class="col-start-1 col-end-1 row-start-2 row-end-2"
          onClick={() => pan(100, 0)}
          title="Pan left"
        >
          <TbChevronLeft />
        </GraphControlButton>
        <GraphControlButton
          class="col-start-3 col-end-3 row-start-2 row-end-2"
          onClick={() => pan(-100, 0)}
          title="Pan right"
        >
          <TbChevronRight />
        </GraphControlButton>
        <GraphControlButton
          class="col-start-2 col-end-2 row-start-3 row-end-3"
          onClick={() => pan(0, -100)}
          title="Pan down"
        >
          <TbChevronDown />
        </GraphControlButton>

        {/* reset */}
        <GraphControlButton
          class="col-start-2 col-end-2 row-start-2 row-end-2"
          onClick={reset}
          title="Reset view"
        >
          <TbRefresh />
        </GraphControlButton>
      </div>
    </div>
  );
}
