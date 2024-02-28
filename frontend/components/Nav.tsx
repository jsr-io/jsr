// Copyright 2024 the JSR authors. All rights reserved. MIT license.
import { ComponentChildren } from "preact";

export interface NavProps {
  children?: ComponentChildren;
}

export function Nav(props: NavProps) {
  return (
    <nav class="mt-3 border-b border-jsr-cyan-300/30 flex gap-1 flex-col md:flex-row md:max-w-full md:overflow-auto">
      {props.children}
    </nav>
  );
}

export interface NavItemProps {
  href: string;
  active?: boolean;
  children?: ComponentChildren;
}

export function NavItem(props: NavItemProps) {
  return (
    <a
      class={`md:px-4 px-2 py-2 text-sm md:text-base leading-none rounded-t-md md:hover:bg-jsr-cyan-100 md:hover:border-b-2 ${
        props.active
          ? "bg-jsr-cyan-200 md:bg-transparent border-b-2 border-jsr-cyan-700"
          : "border-jsr-cyan-400"
      }`}
      href={props.href}
    >
      {props.children}
    </a>
  );
}
