// Copyright 2024 the JSR authors. All rights reserved. MIT license.
import { ComponentChildren } from "preact";
import { NavOverflow } from "./NavOverflow.tsx";

export interface NavProps {
  children?: ComponentChildren;
  noTopMargin?: boolean;
}

export function Nav(props: NavProps) {
  return (
    <nav
      class={`${
        props.noTopMargin ? "" : "mt-3"
      } border-b border-jsr-cyan-300/30 max-w-full flex justify-between overflow-x-auto items-end`}
    >
      <style
        // deno-lint-ignore react-no-danger
        dangerouslySetInnerHTML={{
          __html:
            "nav:has(#nav-items[data-unattached]) { visibility: hidden; }",
        }}
      />
      <noscript>
        <style
          // deno-lint-ignore react-no-danger
          dangerouslySetInnerHTML={{
            __html:
              "nav:has(#nav-items[data-unattached]) { visibility: visible !important }",
          }}
        />
      </noscript>
      <ul
        id="nav-items"
        data-unattached
        class="flex flex-row *:border-b-0 *:rounded-b-none"
      >
        {props.children}
      </ul>
      <NavOverflow />
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
      class={`md:px-3 px-4 py-2 text-sm md:text-base min-h-10 leading-none rounded-md hover:bg-jsr-cyan-500/30 flex items-center select-none focus:outline-none focus:border-1 focus:border-jsr-cyan-300 focus:ring-1 focus:ring-jsr-cyan-300 focus:ring-opacity-50 ${
        props.active
          ? "bg-jsr-cyan-500/20 border-1 border-jsr-cyan-300/30 font-semibold"
          : ""
      }`}
      data-active={props.active ? "true" : undefined}
      href={props.href}
    >
      {props.children}
    </a>
  );
}
