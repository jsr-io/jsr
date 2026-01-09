// Copyright 2024 the JSR authors. All rights reserved. MIT license.
import { useEffect } from "preact/hooks";
import { TbBrightnessUpFilled, TbMoonFilled } from "tb-icons";
import { useSignal } from "@preact/signals";

export default function DarkModeToggle() {
  const isDark = useSignal(false);

  useEffect(() => {
    const isDarkStored = localStorage.getItem("darkMode");
    const isDarkPreference =
      globalThis.matchMedia("(prefers-color-scheme: dark)").matches;
    const initialDarkMode = isDarkStored === "true" ||
      isDarkStored === null && isDarkPreference;

    isDark.value = initialDarkMode;
    updateTheme(initialDarkMode);

    const mediaQuery = globalThis.matchMedia("(prefers-color-scheme: dark)");
    const handleChange = () => {
      if (localStorage.getItem("darkMode") === null) {
        const newDarkMode = mediaQuery.matches;
        isDark.value = newDarkMode;
        updateTheme(newDarkMode);
      }
    };

    mediaQuery.addEventListener("change", handleChange);
    return () => mediaQuery.removeEventListener("change", handleChange);
  }, []);

  function updateTheme(dark: boolean) {
    if (dark) {
      document.documentElement.classList.add("dark");
    } else {
      document.documentElement.classList.remove("dark");
    }
  }

  function toggleDarkMode() {
    const newDarkMode = !isDark.value;
    isDark.value = newDarkMode;
    updateTheme(newDarkMode);
    localStorage.setItem("darkMode", newDarkMode.toString());
  }

  return (
    <button
      onClick={toggleDarkMode}
      class="p-2 rounded-md text-jsr-cyan-700 hover:text-jsr-cyan-900 dark:text-cyan-400 dark:hover:text-cyan-300 hover:bg-jsr-gray-100 dark:hover:bg-jsr-gray-900 focus:outline-hidden"
      title={isDark ? "Switch to light mode" : "Switch to dark mode"}
      aria-label={isDark ? "Switch to light mode" : "Switch to dark mode"}
      type="button"
    >
      {isDark.value
        ? <TbBrightnessUpFilled class="size-5" />
        : <TbMoonFilled class="size-5" />}
    </button>
  );
}
