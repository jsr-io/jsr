// Copyright 2024 the JSR authors. All rights reserved. MIT license.
import type {
  DocsMainContent,
  PackageVersionWithUser,
  User,
} from "../../../utils/api_types.ts";
import { LocalSymbolSearch } from "../(_islands)/LocalSymbolSearch.tsx";
import { Docs } from "../../../util.ts";
import { BreadcrumbsSticky } from "../(_islands)/BreadcrumbsSticky.tsx";
import { TicketModal } from "../../../islands/TicketModal.tsx";
import { TbFlag } from "tb-icons";
import { ModuleDoc, SymbolGroup, Toc } from "../../../components/doc/mod.ts";
import { AllSymbols } from "../../../components/doc/AllSymbols.tsx";
import { ComponentChildren } from "preact";
import DiffVersionSelector from "../(_islands)/DiffVersionSelector.tsx";
import { compileDocsRequestPath, DocsRequest } from "../../../utils/data.ts";

interface DocsProps {
  docs: Docs;
  selectedVersion: PackageVersionWithUser;
  showProvenanceBadge?: boolean;
  user: User | null;
  scope: string;
  pkg: string;
}

interface DiffProps {
  docs: Docs | null;
  versions: PackageVersionWithUser[];
  scope: string;
  pkg: string;
  oldVersion?: string;
  newVersion?: string;
  url: URL;
  request: DocsRequest;
}

interface ProvenanceBadgeProps {
  rekorLogId: string;
}

function ProvenanceBadge({ rekorLogId }: ProvenanceBadgeProps) {
  return (
    <div class="mt-8 mb-8 border-2 border-jsr-cyan-700 max-w-xl rounded-md py-4 px-6">
      <div class="flex flex-row items-end justify-between">
        <div class="items-center">
          <span class="text-sm text-secondary">
            Built and signed on
          </span>

          <div class="flex gap-2 items-center">
            <span class="text-md md:text-2xl font-bold">
              GitHub Actions
            </span>
            <div class="flex items-start lg:items-center gap-2">
              <svg
                aria-hidden="true"
                role="img"
                class="text-green-600 dark:text-green-500 size-4 md:size-5"
                viewBox="0 0 16 16"
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
                href={`https://search.sigstore.dev/?logIndex=${rekorLogId}`}
                target="_blank"
                rel="noopener noreferrer"
                class="jsr-link text-xs lg:text-sm"
              >
                View transparency log
              </a>
            </div>
          </div>
        </div>
      </div>
    </div>
  );
}

function SharedView({
  docs,
  navRightClass,
  navRight,
  children,
  toc,
}: {
  docs: Docs;
  navRightClass?: string;
  navRight: ComponentChildren;
  children?: ComponentChildren;
  toc: ComponentChildren;
}) {
  return (
    <div class="pt-6 pb-8">
      <style
        hidden
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
        <BreadcrumbsSticky content={docs.breadcrumbs} class={navRightClass}>
          {navRight}
        </BreadcrumbsSticky>
      )}

      <div class="grid grid-cols-1 lg:grid-cols-10 gap-8 lg:gap-12">
        <div class="min-w-0 lg:col-span-7 lg:row-start-1 mt-4">
          <div class="ddoc mb-20" id="docMain">
            <MainDocs content={docs.main} />
          </div>
          <div class="ddoc hidden mb-20" id="docSearchResults" />

          {children}
        </div>
        <div
          class={`max-lg:row-start-1 lg:col-[span_3/_-1] lg:sticky flex flex-col box-border gap-y-4 ${
            docs.breadcrumbs
              ? "lg:top-[var(--breadcrumbs-height,0px)] lg:max-h-[calc(100vh-var(--breadcrumbs-height,0px))] -mt-4 lg:mt-0"
              : "lg:top-0 lg:max-h-screen -mt-4 pt-4"
          }`}
        >
          {toc}
        </div>
      </div>
    </div>
  );
}

export function DocsView({
  docs,
  selectedVersion,
  showProvenanceBadge,
  user,
  scope,
  pkg,
}: DocsProps) {
  return (
    <SharedView
      docs={docs}
      navRightClass="lg:col-span-7"
      navRight={
        <div class="lg:col-[span_3/_-1]">
          <LocalSymbolSearch
            content={docs.main.kind === "allSymbols"
              ? docs.main.value
              : undefined}
            scope={scope}
            pkg={pkg}
            version={selectedVersion.version}
          />
        </div>
      }
      toc={
        <>
          {!docs.breadcrumbs && selectedVersion && (
            <LocalSymbolSearch
              scope={scope}
              pkg={pkg}
              version={selectedVersion.version}
            />
          )}
          <Toc content={docs.toc} />
        </>
      }
    >
      <div class="flex justify-between lg:flex-nowrap flex-wrap items-center gap-4">
        {showProvenanceBadge && selectedVersion &&
          selectedVersion.rekorLogId && (
          <ProvenanceBadge rekorLogId={selectedVersion.rekorLogId} />
        )}

        <div>
          <TicketModal
            user={user}
            kind="package_report"
            style="danger"
            title="Report package"
            description={
              <>
                <p className="mt-4 text-secondary">
                  Please provide a reason for reporting this package. We will
                  review your report and take appropriate action.
                </p>
                <p className="mt-4 text-secondary">
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
    </SharedView>
  );
}

export function DiffView({
  docs,
  scope,
  pkg,
  versions,
  oldVersion,
  newVersion,
  url,
  request,
}: DiffProps) {
  const docsRequest = compileDocsRequestPath(request);

  if (!oldVersion || !newVersion || oldVersion == newVersion || !docs) {
    return (
      <div class="mt-7">
        <DiffVersionSelector
          scope={scope}
          pkg={pkg}
          versions={versions.map((version) => version.version)}
          oldVersion={oldVersion}
          newVersion={newVersion}
          url={url}
          docsRequest={docsRequest}
        />
      </div>
    );
  }

  return (
    <SharedView
      docs={docs}
      navRightClass="lg:col-span-5"
      navRight={
        <div class="lg:col-[span_5/_-1]">
          <DiffVersionSelector
            scope={scope}
            pkg={pkg}
            versions={versions.map((version) => version.version)}
            oldVersion={oldVersion}
            newVersion={newVersion}
            url={url}
            docsRequest={docsRequest}
          />
        </div>
      }
      toc={
        <Toc
          content={docs.toc}
          diff={{
            oldVersion,
            oldVersionUrl: `/@${scope}/${pkg}@${oldVersion}/doc${docsRequest}`,
            newVersion,
            newVersionUrl: `/@${scope}/${pkg}@${newVersion}/doc${docsRequest}`,
          }}
        />
      }
    />
  );
}

function MainDocs({ content }: { content: DocsMainContent }) {
  switch (content.kind) {
    case "allSymbols":
      return <AllSymbols items={content.value.entrypoints} />;
    case "file":
    case "index":
      return <ModuleDoc content={content.value} />;
    case "symbol":
      return <SymbolGroup content={content.value} />;
  }
}
