// Copyright 2024 the JSR authors. All rights reserved. MIT license.
import { useEffect, useLayoutEffect, useRef, useState } from "preact/hooks";
import IconDots from "$tabler_icons/dots.tsx";

const useWindowWidth = () => {
  const [windowWidth, setWindowWidth] = useState(0);
  const handleWidth = () => {
    setWindowWidth(globalThis.innerWidth);
  };
  useLayoutEffect(() => {
    handleWidth();
    globalThis.addEventListener("resize", handleWidth);

    return () => globalThis.removeEventListener("resize", handleWidth);
  }, []);
  return windowWidth;
};

export function NavMenu() {
  const [open, setOpen] = useState(false);
  const ref = useRef<HTMLDivElement>(null);

  useWindowWidth();
  useEffect(() => {
    function outsideClick(e: Event) {
      if (ref.current && !ref.current.contains(e.target as Element)) {
        setOpen(false);
      }
    }
    document.addEventListener("click", outsideClick);
    return () => document.removeEventListener("click", outsideClick);
  }, []);

  if (typeof document === "undefined") return null;

  const navItems = document.getElementById("nav-items");
  const nav = navItems?.parentElement;
  let navWidth = 0;
  if (nav) {
    navWidth = nav.offsetWidth;
  }

  let sumWidth = 50;
  let displayMenu = false;
  const navMenuList = [];
  if (navItems) {
    for (let i = 0; i < navItems.children.length; i++) {
      const child = navItems.children[i];
      child.classList.remove("invisible");
      sumWidth += child.clientWidth;

      if (sumWidth > navWidth) {
        displayMenu = true;
        navMenuList.push(child.outerHTML);
        child.classList.add("invisible");
      }
    }
  }

  return (
    <div
      id="nav-menu"
      class={`group absolute right-4 md:right-10 rounded border-2 border-jsr-cyan-200 hover:bg-jsr-cyan-50 hover:cursor-pointer ${
        displayMenu ? "" : "hidden"
      }`}
      aria-expanded={open ? "true" : "false"}
      onClick={() => setOpen((v) => !v)}
      ref={ref}
    >
      <span class="flex p-1">
        <IconDots />
      </span>
      {open && (
        <div
          class="absolute top-[120%] -right-4 z-[70] px-1 py-2 rounded border-1.5 border-current bg-white w-56 shadow overflow-hidden opacity-100 translate-y-0 transition"
          dangerouslySetInnerHTML={{ __html: navMenuList.join("") }}
        />
      )}
    </div>
  );
}
