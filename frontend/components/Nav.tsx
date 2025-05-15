// Copyright 2024 the JSR authors. All rights reserved. MIT license.
import { ComponentChildren } from "preact";
import { NavOverflow } from "./NavOverflow.tsx";

export interface NavProps {
  children?: ComponentChildren;
  end?: ComponentChildren; 
  noTopMargin?: boolean;
}

export function Nav(props: NavProps) {
  return (
    <nav
      class={`${
        props.noTopMargin ? "" : "mt-3"
      } border-b border-jsr-cyan-300/30 dark:border-jsr-cyan-600/50 max-w-full flex justify-between overflow-x-auto items-end`}
      id="nav-items"
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
      <style
        // deno-lint-ignore react-no-danger
        dangerouslySetInnerHTML={{
          __html: `
            .nav-search-item {
              margin-left: auto;
              padding: 0 !important;
              background: none !important;
              border: none !important;
              font-weight: normal !important;
            }
            .nav-search-item:hover {
              background: none !important;
            }
          `,
        }}
      />
      <ul
        data-unattached
        class="flex flex-row *:border-b-0 *:rounded-b-none w-full"
      >
        {props.children}
      </ul>
      {props.end && <div className="ml-auto">{props.end}</div>}
      <NavOverflow />
    </nav>
  );
}

export interface NavItemProps {
  href: string;
  active?: boolean;
  chip?: number;
  notification?: boolean;
  children?: ComponentChildren;
  className?: string;
}

export function NavItem(props: NavItemProps) {
  return (
    <a
      class={`md:px-3 px-4 py-2 text-sm md:text-base min-h-10 leading-none rounded-md hover:bg-jsr-cyan-100 dark:hover:bg-jsr-cyan-900 flex items-center select-none focus:outline-none focus-visible:outline-1 focus-visible:outline-jsr-cyan-300 dark:focus-visible:outline-jsr-cyan-600 focus-visible:outline-offset-0 focus-visible:ring-0 ${
        props.active
          ? "bg-jsr-cyan-50 dark:bg-jsr-cyan-950 border-1 border-jsr-cyan-300/30 dark:border-jsr-cyan-600/50 font-semibold"
          : ""
      } ${props.className || ""}`}
      data-active={props.active ? "true" : undefined}
      href={props.href}
    >
      <span className="flex items-center">
        {props.children}

        {props.chip !== undefined && (
          <span
            className={`chip ml-2 tabular-nums border-1 border-white dark:border-jsr-gray-950 ${
              (props.chip > 0 && props.notification)
                ? "bg-orange-600 text-white"
                : "bg-jsr-gray-200 dark:bg-jsr-gray-900 dark:text-gray-300"
            }`}
          >
            {props.chip}
          </span>
        )}
      </span>
    </a>
  );
}