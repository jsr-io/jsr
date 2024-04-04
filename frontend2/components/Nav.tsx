// Copyright 2024 the JSR authors. All rights reserved. MIT license.
import { ComponentChildren } from "preact";

export interface NavProps {
  children?: ComponentChildren;
}

export function Nav(props: NavProps) {
  return (
    <nav class="mt-3 md:border-b border-jsr-cyan-300/30 flex flex-wrap md:flex-nowrap flex-row max-w-full overflow-auto items-end">
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
      class={`md:px-3 px-4 py-2 text-sm md:text-base min-h-10 leading-none rounded-md md:rounded-t-md md:rounded-b-none hover:bg-jsr-cyan-100 flex items-center select-none ${
        props.active
          ? "bg-jsr-cyan-50 md:border-r-1 md:border-t-1 md:border-l-1 md:border-b-0 border-1 border-jsr-cyan-300/30 font-semibold"
          : ""
      }`}
      href={props.href}
    >
      {props.children}
    </a>
  );
}
