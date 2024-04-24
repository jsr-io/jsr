// Copyright 2024 the JSR authors. All rights reserved. MIT license.
import { Head } from "$fresh/src/runtime/head.ts";
import type { PackageVersionWithUser } from "../../../utils/api_types.ts";
import { LocalSymbolSearch } from "../(_islands)/LocalSymbolSearch.tsx";
import { Docs } from "../../../util.ts";
import { Params } from "./PackageNav.tsx";

interface DocsProps {
  docs: Docs;
  params: Params;
  selectedVersion: PackageVersionWithUser;
  showProvenanceBadge?: boolean;
}

export function DocsView(
  { docs, params, selectedVersion, showProvenanceBadge }: DocsProps,
) {
  const content = (
    <div class="flex-1 min-w-0 pt-4">
      <Head>
        <style dangerouslySetInnerHTML={{ __html: docs.css }} />
        <script dangerouslySetInnerHTML={{ __html: docs.script }} defer />
      </Head>

      {docs.breadcrumbs && (
        <div class="flex lg:items-center justify-between mb-4 md:mb-8 gap-4 max-lg:flex-col-reverse">
          <div
            class="ddoc"
            dangerouslySetInnerHTML={{ __html: docs.breadcrumbs }}
          />

          <LocalSymbolSearch
            scope={params.scope}
            pkg={params.package}
            version={selectedVersion.version}
          />
        </div>
      )}

      <div class="ddoc" dangerouslySetInnerHTML={{ __html: docs.main }} />

      {showProvenanceBadge && selectedVersion.rekorLogId && (
        <div class="mt-8 mb-8 border-2 border-jsr-cyan-500 max-w-xl rounded-md py-4 px-6">
          <div class="flex flex-row items-end justify-between">
            <div class="items-center">
              <span class="text-sm text-jsr-gray-300">
                Built and signed on
              </span>

              <div class="flex gap-2 items-center">
                <span class="text-2xl font-bold">GitHub Actions</span>
                <svg
                  aria-hidden="true"
                  role="img"
                  class="text-green-600"
                  viewBox="0 0 16 16"
                  width="20"
                  height="20"
                  fill="currentColor"
                  style="display: inline-block; user-select: none; vertical-align: text-bottom;"
                >
                  <path
                    fill-rule="evenodd"
                    d="M9.585.52a2.678 2.678 0 00-3.17 0l-.928.68a1.178 1.178 0 01-.518.215L3.83 1.59a2.678 2.678 0 00-2.24 2.24l-.175 1.14a1.178 1.178 0 01-.215.518l-.68.928a2.678 2.678 0 000 3.17l.68.928c.113.153.186.33.215.518l.175 1.138a2.678 2.678 0 002.24 2.24l1.138.175c.187.029.365.102.518.215l.928.68a2.678 2.678 0 003.17 0l.928-.68a1.17 1.17 0 01.518-.215l1.138-.175a2.678 2.678 0 002.241-2.241l.175-1.138c.029-.187.102-.365.215-.518l.68-.928a2.678 2.678 0 000-3.17l-.68-.928a1.179 1.179 0 01-.215-.518L14.41 3.83a2.678 2.678 0 00-2.24-2.24l-1.138-.175a1.179 1.179 0 01-.518-.215L9.585.52zM7.303 1.728c.415-.305.98-.305 1.394 0l.928.68c.348.256.752.423 1.18.489l1.136.174c.51.078.909.478.987.987l.174 1.137c.066.427.233.831.489 1.18l.68.927c.305.415.305.98 0 1.394l-.68.928a2.678 2.678 0 00-.489 1.18l-.174 1.136a1.178 1.178 0 01-.987.987l-1.137.174a2.678 2.678 0 00-1.18.489l-.927.68c-.415.305-.98.305-1.394 0l-.928-.68a2.678 2.678 0 00-1.18-.489l-1.136-.174a1.178 1.178 0 01-.987-.987l-.174-1.137a2.678 2.678 0 00-.489-1.18l-.68-.927a1.178 1.178 0 010-1.394l.68-.928c.256-.348.423-.752.489-1.18l.174-1.136c.078-.51.478-.909.987-.987l1.137-.174a2.678 2.678 0 001.18-.489l.927-.68zM11.28 6.78a.75.75 0 00-1.06-1.06L7 8.94 5.78 7.72a.75.75 0 00-1.06 1.06l1.75 1.75a.75.75 0 001.06 0l3.75-3.75z"
                  >
                  </path>
                </svg>
              </div>
            </div>

            <a
              href={`https://search.sigstore.dev/?logIndex=${selectedVersion.rekorLogId}`}
              target="_blank"
              rel="noopener noreferrer"
              class="text-sm link"
            >
              View transparency log
            </a>
          </div>
        </div>
      )}
    </div>
  );

  if (!docs.sidepanel) {
    return content;
  }

  return (
    <div class="grid grid-cols-1 lg:grid-cols-4 pt-2">
      <div class="col-span-1 top-0 md:pl-0 md:pr-2 pt-4 lg:sticky lg:max-h-screen box-border z-20">
        {!docs.breadcrumbs && (
          <LocalSymbolSearch
            scope={params.scope}
            pkg={params.package}
            version={selectedVersion.version}
          />
        )}
        <div
          class="ddoc w-full lg:*:max-h-[calc(100vh-55px)] b-0"
          dangerouslySetInnerHTML={{ __html: docs.sidepanel }}
        />
      </div>
      <div class="col-span-1 lg:col-span-3">
        {content}
      </div>
    </div>
  );
}
