// Copyright 2024 the JSR authors. All rights reserved. MIT license.
import IconDots from "$tabler_icons/dots.tsx";

const NAV_OVERFLOW_SCRIPT = /* js */ `
(() => {
"use strict";
const navMenuEl = document.getElementById("nav-menu");
const navItemsEl = document.getElementById("nav-items");
const navOverflow = navMenuEl.parentElement;

const navItems = new Map();
for (let i = 0; i < navItemsEl.children.length; i++) {
  const el = navItemsEl.children[i];
  navItems.set(el, el.clientWidth);
}

const active = navItemsEl.querySelector("[data-active]");

function updateNavItems() {
  const navWidth = navItemsEl.parentElement.offsetWidth;
  let sumWidth = 50 + navItems.get(active);
  let displayMenu = false;
  for (const [el, width] of navItems.entries()) {
    if (el !== active) sumWidth += width;
    if (sumWidth > navWidth && el !== active) {
      displayMenu = true;
      navMenuEl.appendChild(el);
    } else {
      navItemsEl.appendChild(el);
    }
  }

  navOverflow.classList[displayMenu ? "remove" : "add"]("hidden");
}

globalThis.addEventListener("resize", () => updateNavItems());
updateNavItems();
navItemsEl.removeAttribute("data-unattached");

let open = false;
function renderOverflowMenuPopup() {
  navMenuEl.setAttribute("aria-expanded", String(open));
  navMenuEl.classList[open ? "remove" : "add"]("hidden");
}

navOverflow.addEventListener("click", () => {
  open = !open;
  renderOverflowMenuPopup();
});

function outsideClick(e) {
  if (navMenuEl.contains(e.target)) {
    open = false;
    renderOverflowMenuPopup();
  }
}
document.addEventListener("click", outsideClick);
})();
`;

export function NavOverflow() {
  return (
    <>
      <button
        type="button"
        class="group absolute right-4 md:right-10 rounded border-1 my-1 border-jsr-cyan-100 hover:bg-jsr-cyan-50 hover:cursor-pointer hidden"
        aria-expanded="false"
      >
        <span class="flex p-1">
          <IconDots />
        </span>
        <div
          id="nav-menu"
          class="absolute top-[120%] -right-2 z-[70] px-1 py-2 rounded border-1.5 border-current bg-white w-56 shadow overflow-hidden opacity-100 translate-y-0 transition [&>a]:rounded hidden"
        />
      </button>
      <script dangerouslySetInnerHTML={{ __html: NAV_OVERFLOW_SCRIPT }} />
    </>
  );
}
