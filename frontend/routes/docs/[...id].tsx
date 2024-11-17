// Copyright 2024 the JSR authors. All rights reserved. MIT license.
import { HttpError } from "fresh";
import { Markdown } from "../../components/Markdown.tsx";
import { define } from "../../util.ts";

import { extract } from "@std/front-matter/yaml";

import TOC, { groupsNames } from "../../docs/toc.ts";

const groups = new Map<string, { id: string; title: string }[]>();
for (const group of groupsNames) {
  groups.set(group, []);
}
export const files = new Map<string, string>();
for (const { id, title, group } of TOC) {
  groups.get(group)!.push({ id, title });
  files.set(id, title);
}

export default define.page<typeof handler>(function Page({ data }) {
  return (
    <div class="mb-20">
      <div class="grid grid-cols-1 md:grid-cols-10">
        <nav class="pb-10 md:border-r-1.5 md:col-span-3 lg:col-span-2 order-2 md:order-1 border-t-1.5 border-jsr-cyan-900 md:border-t-0 md:border-slate-300 pt-4 md:pt-0">
          <div>
            <p class="text-xl font-semibold" id="sidebar">Docs</p>
          </div>

          {Array.from(groups.entries()).map(([group, files]) => (
            <div class="my-6">
              <p class="font-bold">{group}</p>
              <ul class="my-2">
                {files.map(({ id, title }) => (
                  <li>
                    <a
                      href={`/docs/${id}`}
                      class={`${
                        id === data.id
                          ? "px-4 text-jsr-cyan-700 border-l-4 border-jsr-cyan-400 bg-jsr-cyan-100"
                          : "pl-5 pr-4"
                      } py-1.5 block leading-5 hover:text-jsr-gray-600 hover:underline`}
                    >
                      {title}
                    </a>
                  </li>
                ))}
              </ul>
            </div>
          ))}
        </nav>

        <div class="md:col-span-7 mb-12 md:px-6 lg:px-8 order-1 md:order-2">
          <p class="text-sm mb-6 -mt-2 md:hidden">
            <a href="#sidebar" class="link">View table of contents</a>
          </p>
          <h1 class="text-4xl lg:text-5xl lg:leading-[1.1] text-balance font-medium mb-8 text-jsr-gray-900">
            {data.title}
          </h1>
          <Markdown source={data.content} />
          <p class="mt-6 text-sm">
            <a
              class="link"
              href={`https://github.com/jsr-io/jsr/blob/main/frontend/docs/${data.id}.md`}
              target="_blank"
              rel="noopener noreferrer"
            >
              Edit this page on GitHub
            </a>
          </p>
        </div>
      </div>
    </div>
  );
});

export const handler = define.handlers({
  async GET(ctx) {
    ctx.state.searchKind = "docs";

    const { id } = ctx.params;
    if (!files.has(id)) {
      throw new HttpError(404, "This docs page was not found.");
    }

    const path = new URL(`../../docs/${id}.md`, import.meta.url);
    const markdown = await Deno.readTextFile(path);

    const { body, attrs } = extract<{ title: string; description: string }>(
      markdown,
    );
    const title = attrs.title as string ?? files.get(id)!;

    ctx.state.meta = {
      title: `${title} - Docs - JSR`,
      description: attrs.description as string,
    };
    return {
      data: {
        content: body,
        id,
        title,
      },
    };
  },
});
