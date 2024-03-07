// Copyright 2024 the JSR authors. All rights reserved. MIT license.
import { Handlers, PageProps, RouteConfig } from "$fresh/server.ts";
import type {
  Package,
  PackageVersionWithUser,
  SourceDirEntry,
} from "../../utils/api_types.ts";
import { ScopeMember } from "../../utils/api_types.ts";
import { type Source, State } from "../../util.ts";
import { Head } from "$fresh/src/runtime/head.ts";
import { packageDataWithSource } from "../../utils/data.ts";
import { PackageNav, Params } from "./(_components)/PackageNav.tsx";
import { PackageHeader } from "./(_components)/PackageHeader.tsx";
import { Folder } from "../../components/icons/Folder.tsx";
import { Source as SourceIcon } from "../../components/icons/Source.tsx";
import { ListDisplay } from "../../components/List.tsx";

interface Data {
  package: Package;
  selectedVersion: PackageVersionWithUser;
  source: Source | null;
  member: ScopeMember | null;
  sourcePath: string;
}

export default function PackagePage(
  { data, params, state }: PageProps<Data, State>,
) {
  const isStaff = state.user?.isStaff || false;
  const canPublish = data.member !== null || isStaff;
  const canEdit = data.member?.isAdmin || isStaff;

  const sourceRoot =
    `/@${params.scope}/${params.package}/${data.selectedVersion.version}`;

  return (
    <div class="mb-20">
      <Head>
        <title>
          @{params.scope}/{params.package} - JSR
        </title>
        {data.source?.css && (
          <style dangerouslySetInnerHTML={{ __html: data.source.css }} />
        )}
        <meta
          name="description"
          content={`@${params.scope}/${params.package} on JSR${
            data.package.description ? `: ${data.package.description}` : ""
          }`}
        />
      </Head>

      <PackageHeader
        package={data.package}
        selectedVersion={data.selectedVersion ?? undefined}
      />
      <PackageNav
        currentTab="Files"
        versionCount={data.package.versionCount}
        canPublish={canPublish}
        canEdit={canEdit}
        params={params as unknown as Params}
        latestVersion={data.package.latestVersion}
      />

      <div class="space-y-3 pt-3">
        <div class="pl-2">
          {data.sourcePath.split("/").filter((part, i) =>
            !(part === "" && i !== 0)
          ).map((part, i, arr) => {
            if (part === "") {
              // @ts-ignore ok
              part = <span class="italic">root</span>;
            }
            return (
              <>
                {i !== 0 && (
                  <span class="px-1.5 text-xs text-gray-600 select-none">
                    &#x25B6;
                  </span>
                )}

                {(i + 1) < arr.length
                  ? (
                    <a
                      class="link"
                      href={sourceRoot + arr.slice(0, i + 1).join("/")}
                    >
                      {part}
                    </a>
                  )
                  : <span>{part}</span>}
              </>
            );
          })}
        </div>

        {data.source
          ? (
            data.source.source.kind == "dir"
              ? (
                <ListDisplay>
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
                    <div class="ddoc border border-cyan-300 rounded">
                      <div
                        class="markdown children:!bg-transparent"
                        dangerouslySetInnerHTML={{
                          __html: data.source.source.view,
                        }}
                      />
                    </div>
                  )
                  : <i>Source can not be displayed.</i>
              )
          )
          : <i>Source does not exist.</i>}
      </div>
    </div>
  );
}

function DirEntry({ entry }: { entry: SourceDirEntry }) {
  return (
    <div class="grow-1 flex justify-between items-center w-full">
      <div class="flex items-center gap-2">
        <div class="text-gray-500">
          {entry.kind === "dir" ? <Folder /> : <SourceIcon />}
        </div>
        <div class="text-cyan-700 font-semibold">
          {entry.name}
        </div>
      </div>
      <div class="text-sm text-gray-600">
        {bytesToSize(entry.size)}
      </div>
    </div>
  );
}

function bytesToSize(bytes: number) {
  const sizes = ["B", "KB", "MB", "GB", "TB"];
  if (bytes == 0) return "0 B";
  const i = Math.floor(Math.log(bytes) / Math.log(1024));
  return (bytes / Math.pow(1024, i)).toFixed(0) + " " + sizes[i];
}

const LINE_COL_REGEX = /(.*):(\d+):(\d+)$/;

export const handler: Handlers<Data, State> = {
  async GET(_, ctx) {
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
    if (res === null) return ctx.renderNotFound();

    const {
      pkg,
      scopeMember,
      selectedVersion,
      source,
    } = res;

    return ctx.render({
      package: pkg,
      selectedVersion,
      source,
      sourcePath,
      member: scopeMember,
    }, {
      headers: { ...(ctx.params.version ? { "X-Robots-Tag": "noindex" } : {}) },
    });
  },
};

export const config: RouteConfig = {
  routeOverride:
    "/@:scope/:package/:version((?:\\d+\\.\\d+\\.\\d+.*?)|meta\.json)/:path*",
};
