// Copyright 2024 the JSR authors. All rights reserved. MIT license.
import { useEffect, useState } from "preact/hooks";
import { TbBrightnessUpFilled, TbMoonFilled } from "tb-icons";

export default function DarkModeToggle() {
  const [isDark, setIsDark] = useState(false);

  useEffect(() => {
    const isDarkStored = localStorage.getItem("darkMode") === "true";
    const isDarkPreference =
      globalThis.matchMedia("(prefers-color-scheme: dark)").matches;
    const initialDarkMode = isDarkStored ?? isDarkPreference;

    setIsDark(initialDarkMode);
    updateTheme(initialDarkMode);

    const mediaQuery = globalThis.matchMedia("(prefers-color-scheme: dark)");
    const handleChange = () => {
      if (localStorage.getItem("darkMode") === null) {
        const newDarkMode = mediaQuery.matches;
        setIsDark(newDarkMode);
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
    const newDarkMode = !isDark;
    setIsDark(newDarkMode);
    updateTheme(newDarkMode);
    localStorage.setItem("darkMode", newDarkMode.toString());
  }

  return (
    <button
      onClick={toggleDarkMode}
      class="p-2 rounded-md text-primary hover:bg-jsr-gray-100 dark:hover:bg-jsr-gray-900 focus:outline-none"
      title={isDark ? "Switch to light mode" : "Switch to dark mode"}
      aria-label={isDark ? "Switch to light mode" : "Switch to dark mode"}
      type="button"
    >
      {isDark
        ? <TbBrightnessUpFilled class="size-5" />
        : <TbMoonFilled class="size-5" />}
    </button>
  );
}
