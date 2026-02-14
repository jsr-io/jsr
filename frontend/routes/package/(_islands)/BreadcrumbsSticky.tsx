// Copyright 2024 the JSR authors. All rights reserved. MIT license.
import { useSignal } from "@preact/signals";
import { useEffect, useRef } from "preact/hooks";
import { Breadcrumbs } from "../../../components/doc/Breadcrumbs.tsx";
import type { BreadcrumbsCtx } from "@deno/doc/html-types";
import { ComponentChildren } from "preact";

export interface BreadcrumbsStickyProps {
  content: BreadcrumbsCtx;
  class?: string;
  children: ComponentChildren;
}

export function BreadcrumbsSticky(
  props: BreadcrumbsStickyProps,
) {
  const sticky = useSignal(false);
  const ref = useRef<HTMLDivElement>(null);

  useEffect(() => {
    const scrollCb = () => {
      if (ref.current) {
        const rect = ref.current.getBoundingClientRect();

        if (rect.top <= 0) {
          sticky.value = true;
        } else {
          sticky.value = false;
        }
      }
    };

    document.addEventListener("scroll", scrollCb);
    scrollCb();

    return () => document.removeEventListener("scroll", scrollCb);
  }, []);

  return (
    <div
      ref={ref}
      class={`-section-x-inset-xl top-0 sticky bg-white dark:bg-jsr-gray-950 z-20 -my-3 py-3 ${
        sticky.value
          ? "border-b border-jsr-cyan-100 dark:border-jsr-cyan-900 shadow-[0px_2px_4px_0px_rgba(209,235,253,0.40)] dark:shadow-[0px_2px_4px_0px_rgba(50,55,61,0.40)]"
          : ""
      }`}
    >
      <div class="section-x-inset-xl flex md:items-center justify-between gap-4 max-md:flex-col-reverse lg:grid lg:grid-cols-10 lg:gap-12">
        <div class={`ddoc ${props.class ?? ""}`}>
          <Breadcrumbs breadcrumbs={props.content} />
        </div>

        {props.children}
      </div>
    </div>
  );
}
