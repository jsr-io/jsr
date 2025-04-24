// Copyright 2024 the JSR authors. All rights reserved. MIT license.
import type { PackageVersionWithUser, User } from "../../../utils/api_types.ts";
import { LocalSymbolSearch } from "../(_islands)/LocalSymbolSearch.tsx";
import { Docs } from "../../../util.ts";
import { Params } from "./PackageNav.tsx";
import { BreadcrumbsSticky } from "../(_islands)/BreadcrumbsSticky.tsx";
import { TicketModal } from "../../../islands/TicketModal.tsx";
import { TbFlag } from "tb-icons";

interface DocsProps {
  docs: Docs;
  params: Params;
  selectedVersion: PackageVersionWithUser;
  showProvenanceBadge?: boolean;
  user: User | null;
  scope: string;
  pkg: string;
}

const USAGE_SELECTOR_SCRIPT = `(() => {
const preferredUsage = localStorage.getItem('preferredUsage');

if (preferredUsage) {
  document.querySelectorAll('input[name="usage"]').forEach((el) => {
    if (el.id === preferredUsage) el.checked = true;
  });
}

document.querySelector('.usages').addEventListener('change', (e) => {
  const target = e.target;
  if (target instanceof HTMLInputElement && target.name === 'usage') {
    localStorage.setItem('preferredUsage', target.id);
  } 
});
})()`;

export function DocsView({
  docs,
  params,
  selectedVersion,
  showProvenanceBadge,
  user,
  scope,
  pkg,
}: DocsProps) {
  return (
    <div class="pt-6 pb-8 space-y-8">
      <style
        hidden
        // deno-lint-ignore react-no-danger
        dangerouslySetInnerHTML={{ __html: docs.css }}
      />
      <style
        // deno-lint-ignore react-no-danger
        dangerouslySetInnerHTML={{ __html: docs.comrakCss }}
      />
      <script
        hidden
        // deno-lint-ignore react-no-danger
        dangerouslySetInnerHTML={{ __html: docs.script }}
        defer
      />

      {docs.breadcrumbs && (
        <BreadcrumbsSticky
          searchContent={!docs.toc ? docs.main : undefined}
          content={docs.breadcrumbs}
          scope={params.scope}
          package={params.package}
          version={selectedVersion.version}
        />
      )}

      <div class="grid grid-cols-1 lg:grid-cols-10 gap-8 lg:gap-12">
        <div
          class={`min-w-0 ${
            docs.toc ? "lg:col-span-7 lg:row-start-1" : "col-span-full"
          }`}
        >
          <div
            class="ddoc"
            id="docMain"
            // deno-lint-ignore react-no-danger
            dangerouslySetInnerHTML={{ __html: docs.main }}
          />
          <div class="ddoc hidden" id="docSearchResults" />

          <div class="flex justify-between lg:flex-nowrap flex-wrap items-center gap-4">
            {showProvenanceBadge && selectedVersion.rekorLogId && (
              <div class="mt-8 mb-8 border-2 border-jsr-cyan-500 max-w-xl rounded-md py-4 px-6">
                <div class="flex flex-row items-end justify-between">
                  <div class="items-center">
                    <span class="text-sm text-jsr-gray-300">
                      Built and signed on
                    </span>

                    <div class="flex gap-2 items-center">
                      <span class="text-2xl font-bold">GitHub Actions</span>
                      <div class="flex items-start lg:items-center gap-1">
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
                  </div>
                </div>
              </div>
            )}

            <div>
              <TicketModal
                user={user}
                kind="package_report"
                style="danger"
                title="Report package"
                description={
                  <>
                    <p className="mt-4 text-jsr-gray-600">
                      Please provide a reason for reporting this package. We
                      will review your report and take appropriate action.
                    </p>
                    <p className="mt-4 text-jsr-gray-600">
                      Please review the{" "}
                      <a href="/docs/usage-policy#package-contents-and-metadata">
                        JSR usage policy
                      </a>{" "}
                      before submitting a report.
                    </p>
                  </>
                }
                fields={[
                  {
                    name: "message",
                    label: "Reason",
                    type: "textarea",
                    required: true,
                  },
                ]}
                extraMeta={{
                  scope,
                  name: pkg,
                  version: selectedVersion?.version,
                }}
              >
                <TbFlag class="size-6 md:size-4" /> Report package
              </TicketModal>
            </div>
          </div>
        </div>
        {docs.toc && (
          <div
            class={`max-lg:row-start-1 lg:col-[span_3/_-1] lg:top-0 lg:sticky lg:max-h-screen flex flex-col box-border gap-y-4 -mt-4 pt-4 ${
              docs.breadcrumbs ? "lg:-mt-20 lg:pt-20" : ""
            }`}
          >
            {!docs.breadcrumbs && (
              <LocalSymbolSearch
                scope={params.scope}
                pkg={params.package}
                version={selectedVersion.version}
              />
            )}

            <div
              class="ddoc w-full lg:overflow-y-auto pb-4"
              // deno-lint-ignore react-no-danger
              dangerouslySetInnerHTML={{ __html: docs.toc }}
            />
            <script
              // deno-lint-ignore react-no-danger
              dangerouslySetInnerHTML={{ __html: USAGE_SELECTOR_SCRIPT }}
            />
          </div>
        )}
      </div>
    </div>
  );
}
