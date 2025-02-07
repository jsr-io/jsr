// Copyright 2024 the JSR authors. All rights reserved. MIT license.
import { useEffect, useId, useRef, useState } from "preact/hooks";
import { FullUser } from "../utils/api_types.ts";
import TbPlus from "@preact-icons/tb/TbPlus";
import IconArrowRight from "$tabler_icons/arrow-right.tsx";

const SHARED_ITEM_CLASSES =
  "flex items-center px-4 py-2.5 focus-visible:ring-2 ring-inset outline-none";
const DEFAULT_ITEM_CLASSES =
  "hover:bg-jsr-cyan-50 focus-visible:bg-jsr-cyan-200 ring-jsr-cyan-700";

const SUDO_CONFIRMATION =
  "Are you sure you want to enable sudo mode? Sudo mode will be enabled for 5 minutes.";

export function UserMenu({ user, sudo, logoutUrl }: {
  user: FullUser;
  sudo: boolean;
  logoutUrl: string;
}) {
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
    <div class="relative select-none" ref={ref}>
      <button
        id={`${prefix}-user-menu`}
        class="flex items-center rounded-full focus-visible:ring-2 ring-inset outline-none *:focus-visible:ring-jsr-cyan-400 *:focus-visible:ring-offset-1"
        onClick={() => setOpen((v) => !v)}
        aria-expanded={open ? "true" : "false"}
      >
        {user.inviteCount !== 0 && (
          <div class="absolute rounded-full bg-orange-600 border-2 box-content border-white -top-0.5 -right-0.5 h-2 w-2" />
        )}
        <img
          class="w-8 aspect-square rounded-full ring-2 ring-offset-1 ring-jsr-cyan-700"
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
        <div class="flex flex-col items-center gap-3 pt-4 pb-3">
          <img
            class="h-16 w-16 rounded-full ring-2 ring-offset-1 ring-jsr-cyan-950"
            src={user.avatarUrl}
            alt=""
          />
          <div class="text-xl font-semibold">{user.name}</div>
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
          {user.isStaff && (
            <button
              onClick={() => {
                if (sudo) {
                  document.cookie = "sudo=;max-age=0;path=/";
                  location.reload();
                } else if (confirm(SUDO_CONFIRMATION)) {
                  document.cookie = "sudo=1;max-age=300;path=/";
                  location.reload();
                }
              }}
              tabIndex={open ? undefined : -1}
              class="bg-red-600 hover:bg-red-400 text-white text-sm py-1 px-3 flex justify-between items-center gap-3 rounded-full mt-2"
            >
              {sudo ? "Disable" : "Enable"} Sudo Mode
            </button>
          )}
        </div>
        <div class="divide-y divide-slate-200">
          <a
            href="/new"
            tabIndex={open ? undefined : -1}
            class={`${SHARED_ITEM_CLASSES} justify-start gap-2 font-bold bg-jsr-yellow border-jsr-yellow hover:bg-jsr-yellow-300 hover:border-jsr-cyan-500 focus-visible:bg-jsr-yellow-300 focus-visible:border-jsr-yellow-300 ring-black`}
          >
            <TbPlus class="w-5 h-5" />
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
