// Copyright 2024 the JSR authors. All rights reserved. MIT license.
import { useEffect, useRef } from "preact/hooks";
import { useSignal } from "@preact/signals";
import { instance, type Viz } from "@viz-js/viz";

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

interface DependencyGraphItem {
  dependency: DependencyGraphKind;
  children: number[];
  size: number | undefined;
  mediaType: string | undefined;
}

export interface DependencyGraphProps {
  dependencies: DependencyGraphItem[];
}

function createDigraph(dependencies: DependencyGraphProps["dependencies"]) {
  return `
digraph "dependencies" {
  node [fontname="Courier", shape="box"]

  ${
    dependencies.map(({ children, dependency }, index) => {
      return [
        [index, renderDependency(dependency)].join(" "),
        ...children.map((child) => `${index} -> ${child}`),
      ].filter(Boolean).join("\n");
    }).join("\n")
  }
}`;
}

function renderDependency(dependency: DependencyGraphKind) {
  switch (dependency.type) {
    case "jsr":
      return renderJsrDependency(dependency);
    case "npm":
      return renderNpmDependency(dependency);
    case "root":
      return renderRootDependency(dependency);
    case "error":
    default:
      return renderErrorDependency(dependency);
  }
}

function renderJsrDependency(
  dependency: DependencyGraphKindJsr,
) {
  const label =
    `@${dependency.scope}/${dependency.package}@${dependency.version}`;
  const href = `/${label}`;

  return `[href="${href}", label="${label}", tooltip="${label}"]\n`;
}

function renderNpmDependency(dependency: DependencyGraphKindNpm) {
  const label = `${dependency.package}@${dependency.version}`;
  const href = `https://www.npmjs.com/package/${dependency.package}`;

  return `[href="${href}", label="${label}", tooltip="${label}"]\n`;
}

function renderRootDependency(dependency: DependencyGraphKindRoot) {
  const label = dependency.path;

  return `[label="${label}", tooltip="${label}"]\n`;
}

function renderErrorDependency(dependency: DependencyGraphKindError) {
  return ``;
}

export function DependencyGraph(props: DependencyGraphProps) {
  const anchor = useRef<HTMLDivElement>(null);
  const viz = useSignal<Viz | undefined>(undefined);

  useEffect(() => {
    (async () => {
      viz.value = await instance();

      if (anchor.current && viz.value) {
        const digraph = createDigraph(props.dependencies);

        console.log(digraph);

        anchor.current.appendChild(
          viz.value.renderSVGElement(digraph),
        );
      }
    })();
  }, []);

  return (
    <div
      class="-mx-4 md:mx-0 ring-1 ring-jsr-cyan-100 sm:rounded overflow-hidden"
      ref={anchor}
    />
  );
}
