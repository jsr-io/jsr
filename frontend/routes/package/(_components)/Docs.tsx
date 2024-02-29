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
}

export function DocsView({ docs, params, selectedVersion }: DocsProps) {
  const content = (
    <div class="flex-1 min-w-0 px-2 lg:px-6 py-4">
      <Head>
        <style dangerouslySetInnerHTML={{ __html: docs.css }} />
      </Head>

      {docs.breadcrumbs && (
        <div
          class="ddoc"
          dangerouslySetInnerHTML={{ __html: docs.breadcrumbs }}
        />
      )}
      <div class="ddoc" dangerouslySetInnerHTML={{ __html: docs.main }} />

      {selectedVersion.rekorLogId && (
        <div class="mt-4 border border-gray-400 rounded-md py-1 px-2">
          <div className="items-center">
            <span className="text-sm text-gray-600 mr-1">
              Built and signed on
            </span>

            <div className="flex items-center">
              <svg
                aria-hidden="true"
                role="img"
                class="text-green-600"
                viewBox="0 0 16 16"
                width="18"
                height="18"
                fill="currentColor"
                style="display: inline-block; user-select: none; vertical-align: text-bottom;"
              >
                <path
                  fill-rule="evenodd"
                  d="M9.585.52a2.678 2.678 0 00-3.17 0l-.928.68a1.178 1.178 0 01-.518.215L3.83 1.59a2.678 2.678 0 00-2.24 2.24l-.175 1.14a1.178 1.178 0 01-.215.518l-.68.928a2.678 2.678 0 000 3.17l.68.928c.113.153.186.33.215.518l.175 1.138a2.678 2.678 0 002.24 2.24l1.138.175c.187.029.365.102.518.215l.928.68a2.678 2.678 0 003.17 0l.928-.68a1.17 1.17 0 01.518-.215l1.138-.175a2.678 2.678 0 002.241-2.241l.175-1.138c.029-.187.102-.365.215-.518l.68-.928a2.678 2.678 0 000-3.17l-.68-.928a1.179 1.179 0 01-.215-.518L14.41 3.83a2.678 2.678 0 00-2.24-2.24l-1.138-.175a1.179 1.179 0 01-.518-.215L9.585.52zM7.303 1.728c.415-.305.98-.305 1.394 0l.928.68c.348.256.752.423 1.18.489l1.136.174c.51.078.909.478.987.987l.174 1.137c.066.427.233.831.489 1.18l.68.927c.305.415.305.98 0 1.394l-.68.928a2.678 2.678 0 00-.489 1.18l-.174 1.136a1.178 1.178 0 01-.987.987l-1.137.174a2.678 2.678 0 00-1.18.489l-.927.68c-.415.305-.98.305-1.394 0l-.928-.68a2.678 2.678 0 00-1.18-.489l-1.136-.174a1.178 1.178 0 01-.987-.987l-.174-1.137a2.678 2.678 0 00-.489-1.18l-.68-.927a1.178 1.178 0 010-1.394l.68-.928c.256-.348.423-.752.489-1.18l.174-1.136c.078-.51.478-.909.987-.987l1.137-.174a2.678 2.678 0 001.18-.489l.927-.68zM11.28 6.78a.75.75 0 00-1.06-1.06L7 8.94 5.78 7.72a.75.75 0 00-1.06 1.06l1.75 1.75a.75.75 0 001.06 0l3.75-3.75z"
                >
                </path>
              </svg>
              <span className="text-lg font-bold ml-1">GitHub Actions</span>
            </div>
          </div>

          <a
            href={`https://search.sigstore.dev/?logIndex=${selectedVersion.rekorLogId}`}
            target="_blank"
            rel="noopener noreferrer"
            className="text-sm text-blue-600 hover:underline"
          >
            View transparency log
          </a>
        </div>
      )}
    </div>
  );

  if (!docs.sidepanel) {
    return content;
  }

  return (
    <div class="grid grid-cols-1 lg:grid-cols-4 py-2">
      <div class="col-span-1 top-0 sticky md:pl-0 md:pr-2 max-h-screen py-4 box-border">
        <LocalSymbolSearch
          scope={params.scope}
          pkg={params.package}
          version={selectedVersion.version}
        />
        <div
          class="ddoc w-full min-h-0 *:!h-full"
          dangerouslySetInnerHTML={{ __html: docs.sidepanel }}
        />
      </div>
      <div class="col-span-1 lg:col-span-3">
        {content}
      </div>
    </div>
  );
}
