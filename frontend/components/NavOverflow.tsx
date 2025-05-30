// Copyright 2024 the JSR authors. All rights reserved. MIT license.
import TbDots from "tb-icons/TbDots";

const NAV_OVERFLOW_SCRIPT = /* js */ `
(() => {
  "use strict";
  const navOverflowMenuEl = document.getElementById("nav-overflow-menu");
  const navOverflowButton = navOverflowMenuEl.parentElement;
  
	const navItems = document.getElementById("nav-items");
  const navItemListsEl = document.querySelectorAll("#nav-items > ul");
  
  const navItemsParents = new Map();
  const navItemsWidths = new Map();
	for (const list of navItemListsEl) {
	  for (const el of list.children) {
			navItemsParents.set(el, list);
      navItemsWidths.set(el, el.clientWidth);
	  }
	}

  const active = navItems.querySelector("[data-active]");

  function updateNavItems() {
    const navWidth = navItems.offsetWidth;
    let sumWidth = 50 + navItemsWidths.get(active);
    let displayOverflowMenu = false;
    for (const [el, width] of navItemsWidths.entries()) {
      if (el !== active) sumWidth += width;
      if (sumWidth > navWidth && el !== active) {
        displayOverflowMenu = true;
        navOverflowMenuEl.appendChild(el);
      } else {
				let parent = navItemsParents.get(el);
        parent.appendChild(el);
      }
    }
  
    navOverflowButton.classList[displayOverflowMenu ? "remove" : "add"]("hidden");
  }
  
  globalThis.addEventListener("resize", () => updateNavItems());
  updateNavItems();
  for (const list of navItemListsEl) {
    list.removeAttribute("data-unattached");
  }
		
  let open = false;
  function renderOverflowMenuPopup() {
    navOverflowMenuEl.setAttribute("aria-expanded", String(open));
    navOverflowMenuEl.classList[open ? "remove" : "add"]("hidden");
  }
  
  navOverflowButton.addEventListener("click", (e) => {
    // Check if the click is on an input or if any parent up to the event currentTarget is an input
    let el = e.target;
    while (el && el !== e.currentTarget) {
			console.log(el.tagName);
      if (el.tagName === 'INPUT') return;
      el = el.parentElement;
    }
		
    open = !open;
    renderOverflowMenuPopup();
  });
  
  function outsideClick(e) {
    if (!navOverflowMenuEl.parent.contains(e.target)) {
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
        class="group absolute right-4 md:right-10 rounded border-1 my-1 border-jsr-cyan-100 dark:border-jsr-cyan-800 hover:bg-jsr-cyan-50 dark:hover:bg-jsr-cyan-700 hover:cursor-pointer hidden"
        aria-expanded="false"
      >
        <span class="flex p-1">
          <TbDots class="size-6" />
        </span>
        <div
          id="nav-overflow-menu"
          class="absolute top-[120%] -right-2 z-[70] px-1 py-2 rounded border-1.5 border-current dark:border-cyan-700 bg-white dark:bg-jsr-gray-950 w-56 shadow overflow-hidden opacity-100 translate-y-0 transition [&>a]:rounded hidden"
        />
      </button>
      <script
        // deno-lint-ignore react-no-danger
        dangerouslySetInnerHTML={{ __html: NAV_OVERFLOW_SCRIPT }}
      />
    </>
  );
}
