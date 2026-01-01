// Copyright 2024 the JSR authors. All rights reserved. MIT license.
import { HttpError, RouteConfig } from "fresh";
import type { SourceDirEntry } from "../../utils/api_types.ts";
import { define } from "../../util.ts";
import { packageDataWithSource } from "../../utils/data.ts";
import { PackageNav, Params } from "./(_components)/PackageNav.tsx";
import { PackageHeader } from "./(_components)/PackageHeader.tsx";
import { TbFileOff, TbFolder, TbFolderOpen, TbSourceCode } from "tb-icons";
import { ListDisplay } from "../../components/List.tsx";
import { scopeIAM } from "../../utils/iam.ts";
import { format as formatBytes } from "@std/fmt/bytes";

export default define.page<typeof handler>(function PackagePage(
  { data, params, state },
) {
  const iam = scopeIAM(state, data.member);

  const sourceRoot =
    `/@${params.scope}/${params.package}/${data.selectedVersion.version}`;

  return (
    <div class="mb-20">
      {data.source && (
        <>
          <style
            // deno-lint-ignore react-no-danger
            dangerouslySetInnerHTML={{ __html: data.source.comrakCss }}
          />
          <style
            // deno-lint-ignore react-no-danger
            dangerouslySetInnerHTML={{ __html: data.source.css }}
          />
          <script
            hidden
            // deno-lint-ignore react-no-danger
            dangerouslySetInnerHTML={{ __html: data.source.script }}
            defer
          />
        </>
      )}
      <PackageHeader
        package={data.package}
        selectedVersion={data.selectedVersion ?? undefined}
        downloads={data.downloads}
      />
      <PackageNav
        currentTab="Files"
        versionCount={data.package.versionCount}
        dependencyCount={data.package.dependencyCount}
        dependentCount={data.package.dependentCount}
        iam={iam}
        params={params as unknown as Params}
        latestVersion={data.package.latestVersion}
      />

      <div class="pt-8">
        <div class="ring-1 ring-jsr-cyan-100 dark:ring-jsr-cyan-900 rounded-md overflow-hidden">
          <nav
            aria-label="File navigation"
            class="flex items-center gap-2.5 px-5 py-3 bg-jsr-cyan-50 dark:bg-jsr-cyan-950 border-b border-jsr-cyan-100 dark:border-jsr-cyan-900"
          >
            <TbFolderOpen
              class="text-jsr-cyan-700 dark:text-jsr-cyan-400 size-5 flex-shrink-0"
              aria-hidden="true"
            />
            <div class="flex flex-row flex-wrap gap-1 items-center">
              {data.sourcePath.split("/").filter((part, i) =>
                !(part === "" && i !== 0)
              ).map((part, i, arr) => {
                const isRoot = part === "";
                const displayPart = isRoot ? "Package root" : part;
                const isLast = (i + 1) >= arr.length;

                return (
                  <>
                    {i !== 0 && (
                      <span class="text-secondary select-none" aria-hidden="true">
                        /
                      </span>
                    )}
                    {isLast
                      ? (
                        <span class={isRoot ? "font-semibold" : ""}>
                          {displayPart}
                        </span>
                      )
                      : (
                        <a
                          class={`text-jsr-cyan-700 dark:text-jsr-cyan-400 hover:text-jsr-cyan-900 dark:hover:text-jsr-cyan-300 ${isRoot ? "font-semibold" : ""}`}
                          href={sourceRoot + arr.slice(0, i + 1).join("/")}
                        >
                          {displayPart}
                        </a>
                      )}
                  </>
                );
              })}
            </div>
          </nav>

          {data.source
            ? (
              data.source.source.kind == "dir"
                ? (
                  <ListDisplay hasHeader>
                    {data.source.source.entries.map((entry) => (
                      {
                        href: (sourceRoot +
                          (data.sourcePath === "/" ? "" : data.sourcePath) +
                          "/") + entry.name,
                        content: <DirEntry entry={entry} />,
                      }
                    ))}
                  </ListDisplay>
                )
              : (
                data.source.source.view
                  ? (
                    <div class="ddoc">
                      <div
                        class="markdown ddoc-full children:!bg-transparent"
                        // deno-lint-ignore react-no-danger
                        dangerouslySetInnerHTML={{
                          __html: data.source.source.view,
                        }}
                      />
                    </div>
                  )
                  : (
                    <div class="flex items-center gap-2 px-5 py-4 text-secondary">
                      <TbFileOff class="size-5 flex-shrink-0" aria-hidden="true" />
                      <span>Source cannot be displayed.</span>
                    </div>
                  )
              )
            )
            : (
              <div class="flex items-center gap-2 px-5 py-4 text-secondary">
                <TbFileOff class="size-5 flex-shrink-0" aria-hidden="true" />
                <span>Source does not exist.</span>
              </div>
            )}
        </div>
      </div>
    </div>
  );
});

function DirEntry({ entry }: { entry: SourceDirEntry }) {
  return (
    <div class="grow-1 flex justify-between items-center w-full">
      <div class="flex items-center gap-2">
        <div class="text-tertiary">
          {entry.kind === "dir" ? <TbFolder /> : <TbSourceCode />}
        </div>
        <div class="text-cyan-700 font-semibold">
          {entry.name}
        </div>
      </div>
      <div class="text-sm text-secondary">
        {formatBytes(entry.size, { maximumFractionDigits: 0 }).toUpperCase()}
      </div>
    </div>
  );
}

const LINE_COL_REGEX = /(.*):(\d+):(\d+)$/;

export const handler = define.handlers({
  async GET(ctx) {
    const originalPath = ctx.params.path;
    ctx.params.path = originalPath.replace(LINE_COL_REGEX, "$1#L$2");
    if (originalPath !== ctx.params.path) {
      return new Response("", {
        status: 302,
        headers: {
          "Location":
            `/@${ctx.params.scope}/${ctx.params.package}/${ctx.params.version}/${ctx.params.path}`,
        },
      });
    }
    let sourcePath = "/" + (ctx.params.path ?? "");
    if (ctx.params.version === "meta.json" && ctx.params.path === "") {
      sourcePath = "meta.json";
      ctx.params.version = "latest";
    }
    if (ctx.params.version.endsWith("_meta.json") && ctx.params.path === "") {
      sourcePath = ctx.params.version;
      ctx.params.version = ctx.params.version.slice(0, -10);
    }
    const res = await packageDataWithSource(
      ctx.state,
      ctx.params.scope,
      ctx.params.package,
      ctx.params.version,
      sourcePath,
    );
    if (res === null) {
      throw new HttpError(
        404,
        "This file or this package version was not found.",
      );
    }

    const {
      pkg,
      scopeMember,
      selectedVersion,
      source,
      downloads,
    } = res;

    ctx.state.meta = {
      title: `${sourcePath} - @${pkg.scope}/${pkg.name} - JSR`,
      description: `@${pkg.scope}/${pkg.name} on JSR${
        pkg.description ? `: ${pkg.description}` : ""
      }`,
    };
    return {
      data: {
        package: pkg,
        downloads,
        selectedVersion,
        source,
        sourcePath,
        member: scopeMember,
      },
      headers: { ...(ctx.params.version ? { "X-Robots-Tag": "noindex" } : {}) },
    };
  },
});

export const config: RouteConfig = {
  routeOverride:
    "/@:scope/:package/:version((?:\\d+\\.\\d+\\.\\d+.*?)|meta\.json)/:path*",
};
