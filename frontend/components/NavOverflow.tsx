// Copyright 2024 the JSR authors. All rights reserved. MIT license.
import TbDots from "tb-icons/TbDots";

const NAV_OVERFLOW_SCRIPT = /* js */ `
(() => {
"use strict";
const navMenuEl = document.getElementById("nav-menu");
const navItemsEl = document.getElementById("nav-items");
const navOverflow = navMenuEl.parentElement;
const navList = navItemsEl.querySelector("ul");

// Get all navigation items and their original order
const navItems = [];
for (let i = 0; i < navList.children.length; i++) {
  const el = navList.children[i];
  navItems.push({
    element: el,
    width: el.clientWidth,
    position: i,
    isActive: el.hasAttribute('data-active')
  });
}

function updateNavItems() {
  const navWidth = navItemsEl.offsetWidth - 50; // 50px for the overflow button
  let availableWidth = navWidth;
  let displayMenu = false;
  
  // First, move all non-active items to the menu
  for (const item of navItems) {
    if (!item.isActive) {
      navMenuEl.appendChild(item.element);
    } else {
      availableWidth -= item.width;
    }
  }
  
  // Get items from menu and prepare for sorting
  const menuItems = [...navMenuEl.children];
  const itemsFromMenu = [];
  
  for (const el of menuItems) {
    const itemData = navItems.find(item => item.element === el);
    if (itemData) {
      itemsFromMenu.push(itemData);
    }
  }
  
  itemsFromMenu.sort((a, b) => a.position - b.position);

  // Add items back to navbar from left-to-right until we run out of space
  for (const item of itemsFromMenu) {
    if (item.width <= availableWidth) {
      navList.appendChild(item.element);
      availableWidth -= item.width;
    } else {
      displayMenu = true;
    }
  }
  
  navOverflow.classList[displayMenu ? "remove" : "add"]("hidden");
  
  // Now sort items in the overflow menu to match the original order
  const overflowItems = [...navMenuEl.children];
  
  // First remove all items from the menu
  while (navMenuEl.firstChild) {
    navMenuEl.removeChild(navMenuEl.firstChild);
  }
  
  // Sort for menu display: in their original left-to-right order
  overflowItems.sort((a, b) => {
    const itemA = navItems.find(item => item.element === a);
    const itemB = navItems.find(item => item.element === b);
    return itemA.position - itemB.position;
  });
  
  // Add them back in the correct order
  for (const el of overflowItems) {
    navMenuEl.appendChild(el);
  }
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
        class="group absolute right-4 md:right-10 rounded border-1 my-1 border-jsr-cyan-100 dark:border-jsr-cyan-800 hover:bg-jsr-cyan-50 dark:hover:bg-jsr-cyan-700 hover:cursor-pointer hidden"
        aria-expanded="false"
      >
        <span class="flex p-1">
          <TbDots class="size-6" />
        </span>
        <div
          id="nav-menu"
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