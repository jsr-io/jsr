// Copyright 2024 the JSR authors. All rights reserved. MIT license.
export interface PanelEntry {
  value: string;
  href: string;
  label?: string;
}

export function ListPanel(
  { title, subtitle, selected, children }: {
    title?: string;
    subtitle?: string;
    selected?: string;
    children: PanelEntry[];
  },
) {
  return (
    <div class="w-full">
      <div class="mb-2">
        {title && (
          <h3 class="text-lg md:text-xl font-semibold">
            {title}
          </h3>
        )}
        {subtitle && (
          <div class="text-base text-jsr-gray-500 dark:text-gray-400">
            {subtitle}
          </div>
        )}
      </div>
      <ol class="border-1.5 border-jsr-cyan-950 dark:border-jsr-gray-600 rounded list-none overflow-hidden">
        {children.map((entry) => {
          return (
            <li class={children.length > 1 ? "odd:bg-jsr-cyan-50 dark:odd:bg-jsr-gray-800/70 even:dark:bg-jsr-gray-900" : "dark:bg-jsr-gray-800/70"}>
              <a
                class={`flex px-4 items-center py-3 group focus-visible:ring-2 ring-jsr-cyan-700 dark:ring-cyan-500 ring-inset outline-none hover:bg-jsr-yellow-200 dark:hover:bg-jsr-gray-700 focus-visible:bg-jsr-yellow-200 dark:focus-visible:bg-jsr-gray-700 ${
                  entry.value === selected ? "text-jsr-cyan-700 dark:text-cyan-400 font-bold" : "dark:text-gray-200"
                }`}
                href={entry.href}
              >
                <span class="block group-hover:text-jsr-cyan-800 dark:group-hover:text-cyan-300 pr-4 flex-1 group-hover:underline truncate">
                  {entry.value}
                </span>
                {entry.label && (
                  <div class="chip bg-jsr-cyan-200 dark:bg-jsr-cyan-900 max-w-20 truncate">
                    {entry.label}
                  </div>
                )}
              </a>
            </li>
          );
        })}
      </ol>
    </div>
  );
}
