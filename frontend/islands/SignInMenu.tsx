// Copyright 2024 the JSR authors. All rights reserved. MIT license.
import { useEffect, useId, useRef } from "preact/hooks";
import { useSignal } from "@preact/signals";
import { TbLogin2 } from "tb-icons";
import { asset } from "fresh/runtime";

const SHARED_ITEM_CLASSES =
  "flex items-center justify-start gap-2 px-4 py-2.5 focus-visible:ring-2 ring-inset outline-none";

export function SignInMenu({ redirect }: { redirect: string }) {
  const open = useSignal(false);
  const ref = useRef<HTMLDivElement>(null);

  useEffect(() => {
    function outsideClick(e: Event) {
      if (ref.current && !ref.current.contains(e.target as Element)) {
        open.value = false;
      }
    }
    document.addEventListener("click", outsideClick);
    return () => document.removeEventListener("click", outsideClick);
  }, []);

  const prefix = useId();

  return (
    <div class="relative select-none" ref={ref}>
      <button
        id={`${prefix}-user-menu`}
        class="flex items-center gap-2 link-header"
        type="button"
        onClick={() => open.value = !open.value}
        aria-expanded={open.value ? "true" : "false"}
      >
        <TbLogin2 class="size-5" />
        Sign In
      </button>
      <div
        aria-labelledby={`${prefix}-user-menu`}
        role="region"
        class={`absolute top-[120%] -right-4 z-[80] rounded border-1.5 border-current bg-white dark:bg-jsr-gray-950 dark:text-gray-200 w-64 shadow overflow-hidden ${
          open.value
            ? "opacity-100 translate-y-0"
            : "opacity-0 translate-y-5 pointer-events-none"
        } transition`}
        style="--tw-shadow-color: rgba(156,163,175,0.2);"
      >
        <div class="divide-y divide-jsr-gray-800">
          <a
            href={"/login/github" + redirect}
            tabIndex={open.value ? undefined : -1}
            class={`${SHARED_ITEM_CLASSES} font-bold bg-jsr-yellow border-jsr-yellow hover:bg-jsr-yellow-300 hover:border-jsr-cyan-500 focus-visible:bg-jsr-yellow-300 focus-visible:border-jsr-yellow-300 ring-black text-jsr-gray-950`}
          >
            <img class="size-5" src={asset(`/logos/github.svg`)} />
            Sign In with GitHub
          </a>
          <a
            href={"/login/gitlab" + redirect}
            tabIndex={open.value ? undefined : -1}
            class={`${SHARED_ITEM_CLASSES} font-bold bg-jsr-yellow border-jsr-yellow hover:bg-jsr-yellow-300 hover:border-jsr-cyan-500 focus-visible:bg-jsr-yellow-300 focus-visible:border-jsr-yellow-300 ring-black text-jsr-gray-950`}
          >
            <img class="size-5" src={asset(`/logos/gitlab.svg`)} />
            Sign In with GitLab
          </a>
        </div>
      </div>
    </div>
  );
}
