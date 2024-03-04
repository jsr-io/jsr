// Copyright 2024 the JSR authors. All rights reserved. MIT license.
import { Handlers, PageProps } from "$fresh/server.ts";
import { Markdown } from "../../components/Markdown.tsx";
import { Head } from "$fresh/src/runtime/head.ts";
import { State } from "../../util.ts";

import { extract } from "$std/front_matter/yaml.ts";

import TOC, { groupsNames } from "../../docs/toc.ts";

const groups = new Map<string, { id: string; title: string }[]>();
for (const group of groupsNames) {
  groups.set(group, []);
}
const files = new Map<string, string>();
for (const { id, title, group } of TOC) {
  groups.get(group)!.push({ id, title });
  files.set(id, title);
}

interface Data {
  id: string;
  title: string;
  description: string;
  content: string;
}

export default function PackagePage({ data }: PageProps<Data, State>) {
  return (
    <div class="mb-20">
      <Head>
        <title>{data.title} - Docs - JSR</title>
        <meta name="description" content={data.description} />
      </Head>

      <div class="grid grid-cols-1 md:grid-cols-10">
        <nav class="pb-10 md:border-r-1.5 md:col-span-3 lg:col-span-2 order-2 md:order-1 border-t-1.5 border-cyan-900 md:border-t-0 md:border-slate-300 pt-4 md:pt-0">
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
                          ? "px-4 text-cyan-700 border-l-4 border-cyan-400 bg-cyan-100"
                          : "pl-5 pr-4"
                      } py-1.5 block leading-5 hover:text-gray-600 hover:underline`}
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
          <h1 class="text-4xl lg:text-5xl lg:leading-[1.1] text-balance font-medium mb-8 text-gray-900">
            {data.title}
          </h1>
          <Markdown source={data.content} />
          <p class="mt-6 text-sm">
            <a
              class="link"
              href={`https://github.com/jsr-io/jsr/blob/main/frontend/docs/${data.id}.md`}
            >
              Edit this page on GitHub
            </a>
          </p>
        </div>
      </div>
    </div>
  );
}

export const handler: Handlers<Data, State> = {
  async GET(_, ctx) {
    const { id } = ctx.params;
    if (!files.has(id)) return ctx.renderNotFound();

    const title = files.get(id)!;
    const path = new URL(`../../docs/${id}.md`, import.meta.url).pathname;
    const markdown = await Deno.readTextFile(path);

    const { body, attrs } = extract(markdown);

    return ctx.render({
      content: body,
      id,
      title: attrs.title as string ?? title,
      description: attrs.description as string,
    });
  },
};
