// Copyright 2024 the JSR authors. All rights reserved. MIT license.
import { useEffect, useId, useRef, useState } from "preact/hooks";
import { FullUser } from "../utils/api_types.ts";
import { Plus } from "../components/icons/Plus.tsx";
import IconArrowRight from "$tabler_icons/arrow-right.tsx";

const SHARED_ITEM_CLASSES =
  "flex items-center px-4 py-2.5 focus-visible:ring-2 ring-inset outline-none";
const DEFAULT_ITEM_CLASSES =
  "hover:bg-jsr-yellow-300 focus-visible:bg-jsr-yellow-200 ring-jsr-yellow-700";

export function UserMenu(
  { user, logoutUrl }: { user: FullUser; logoutUrl: string },
) {
  const [open, setOpen] = useState(false);
  const ref = useRef<HTMLDivElement>(null);

  useEffect(() => {
    function outsideClick(e: Event) {
      if (ref.current && !ref.current.contains(e.target as Element)) {
        setOpen(false);
      }
    }
    document.addEventListener("click", outsideClick);
    return () => document.removeEventListener("click", outsideClick);
  }, []);

  const prefix = useId();

  return (
    <div class="relative" ref={ref}>
      <button
        id={`${prefix}-user-menu`}
        class="flex items-center rounded-full"
        onClick={() => setOpen((v) => !v)}
        aria-expanded={open ? "true" : "false"}
      >
        {user.inviteCount !== 0 && (
          <div class="absolute rounded-full bg-orange-600 border-2 box-content border-white -top-0.5 -right-0.5 h-2 w-2" />
        )}
        <img
          class="w-8 aspect-square rounded-full ring-2 ring-offset-1 ring-jsr-cyan-950"
          src={user.avatarUrl}
          alt={user.name}
        />
      </button>
      <div
        aria-labelledby={`${prefix}-user-menu`}
        role="region"
        class={`absolute top-[120%] -right-4 z-[80] rounded border-1.5 border-current bg-white w-56 shadow overflow-hidden ${
          open
            ? "opacity-100 translate-y-0"
            : "opacity-0 translate-y-5 pointer-events-none"
        } transition`}
        style="--tw-shadow-color: rgba(156,163,175,0.2);"
      >
        <div class="flex flex-col items-center p-4">
          <img
            class="h-24 w-24 rounded-full border-1 border-jsr-gray-0"
            src={user.avatarUrl}
            alt=""
          />
          <div class="text-xl font-semibold mt-2">{user.name}</div>
          {user.inviteCount !== 0 && (
            <a
              class="bg-orange-600 hover:bg-orange-400 text-white text-sm py-1 pl-4 pr-2 flex justify-between items-center gap-3 rounded-full mt-2"
              href="/account/invites"
            >
              <span>
                {user.inviteCount} pending invite{user.inviteCount > 1 && "s"}
              </span>
              <IconArrowRight class="w-4 h-4" />
            </a>
          )}
        </div>
        <div class="divide-y divide-slate-200 border-t">
          <a
            href="/new"
            tabIndex={open ? undefined : -1}
            class={`${SHARED_ITEM_CLASSES} justify-start gap-2 bg-jsr-yellow border-jsr-yellow hover:bg-jsr-yellow-300 hover:border-jsr-yellow-300 focus-visible:bg-jsr-yellow-300 focus-visible:border-jsr-yellow-300 ring-black`}
          >
            <Plus />
            Publish a package
          </a>
          <a
            href={`/user/${user.id}`}
            tabIndex={open ? undefined : -1}
            class={`${SHARED_ITEM_CLASSES} ${DEFAULT_ITEM_CLASSES}`}
          >
            Account
          </a>
          {user.isStaff && (
            <a
              href="/admin"
              tabIndex={open ? undefined : -1}
              class={`${SHARED_ITEM_CLASSES} ${DEFAULT_ITEM_CLASSES}`}
            >
              Admin Panel
            </a>
          )}
          <a
            href={`/logout?redirect=${logoutUrl}`}
            tabIndex={open ? undefined : -1}
            class={`${SHARED_ITEM_CLASSES} ${DEFAULT_ITEM_CLASSES}`}
          >
            Sign out
          </a>
        </div>
      </div>
    </div>
  );
}
