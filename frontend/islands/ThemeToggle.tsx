#!/usr/bin/env -S deno run -A --watch
// Copyright 2024 the JSR authors. All rights reserved. MIT license.

import { useState } from "preact/hooks";
import { IS_BROWSER } from "fresh/runtime";
import { TbMoon, TbSun } from "@preact-icons/tb";
/**
 * The head script string to put in the <Head> element.
 * Important for avoiding FOUC (Flash Of Unstyled Content).`
 */
export const themeToggleHeadScript: string = `
const isDarkMode = localStorage.theme === "dark"
|| (!("theme" in localStorage)
  && window.matchMedia("(prefers-color-scheme: dark)").matches);
document.documentElement.dataset.theme = isDarkMode ? "dark" : "light";
window.onload = function() {
  const themeToggles = document.querySelectorAll(".dark-mode-toggle.hidden");
  themeToggles.forEach((el) => el.classList.remove("hidden"));
};
`.replace(/(\n|\t)/g, "").replace(/"/g, "'");

/**
 * ThemeToggle
 * @island
 *
 * @description
 * It toggles the dark mode based on user preference and updates the localStorage.
 */
export default function ThemeToggle() {
  const [theme, setTheme] = useState(() => {
    if (!IS_BROWSER) return "light";
    return document.documentElement.dataset.theme ?? "light";
  });

  const toggleTheme = () => {
    setTheme((prev) => {
      const theme = prev === "light" ? "dark" : "light";
      document.documentElement.setAttribute("data-theme", theme);
      localStorage.setItem("theme", theme);
      return theme;
    });
  };

  return (
    <button
      type="button"
      onClick={toggleTheme}
      class="dark-mode-toggle button hidden" /* Hidden until JavaScript unhides me */
      aria-label="Toggle Theme"
    >
      {theme === "light"
        ? <TbMoon class="w-6 h-6" />
        : <TbSun class="w-6 h-6" />}
    </button>
  );
}
