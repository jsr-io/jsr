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
          <h2 class="text-xl md:text-2xl font-semibold">
            {title}
          </h2>
        )}
        {subtitle && (
          <div class="text-base text-gray-500">
            {subtitle}
          </div>
        )}
      </div>
      <ol class="border-1.5 border-jsr-cyan-950 rounded list-none overflow-hidden">
        {children.map((entry) => {
          return (
            <li class="odd:bg-jsr-cyan-50">
              <a
                class={`flex px-4 items-center py-3 group focus-visible:ring-2 ring-jsr-cyan-700 ring-inset outline-none hover:bg-jsr-yellow-200 focus-visible:bg-jsr-yellow-200 ${
                  entry.value === selected ? "text-cyan-700 font-bold" : ""
                }`}
                href={entry.href}
              >
                <span class="block w-full truncate group-hover:text-jsr-cyan-800 pr-4 group-hover:underline">
                  {entry.value}
                </span>
                {entry.label && (
                  <div class="chip bg-jsr-cyan-200">
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
