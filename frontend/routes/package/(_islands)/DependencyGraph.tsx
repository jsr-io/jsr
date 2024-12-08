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

interface DependencyGraphKindJsr {
  type: "jsr";
  scope: string;
  package: string;
  version: string;
  path: string;
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

type DependencyGraphKind =
  | DependencyGraphKindJsr
  | DependencyGraphKindNpm
  | DependencyGraphKindRoot
  | DependencyGraphKindError;

export interface DependencyGraphItem {
  dependency: DependencyGraphKind;
  children: number[];
  size: number | undefined;
  mediaType: string | undefined;
}

export interface DependencyGraphProps {
  dependencies: DependencyGraphItem[];
}

function createDigraph(dependencies: DependencyGraphProps["dependencies"]) {
  return `digraph "dependencies" {
  graph [rankdir="LR"]
  node [fontname="Courier", shape="box", style="filled,rounded"]

${
    dependencies.map(({ children, dependency, size }, index) => {
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

function renderDependency(dependency: DependencyGraphKind, size?: number) {
  let href;
  let content;
  let tooltip;
  let color;
  switch (dependency.type) {
    case "jsr": {
      tooltip =
        `@${dependency.scope}/${dependency.package}@${dependency.version}`;
      href = `/${tooltip}`;
      content = `${tooltip}\n${dependency.path}\n${bytesToSize(size ?? 0)}`;
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

function useDigraph(dependencies: DependencyGraphProps["dependencies"]) {
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
