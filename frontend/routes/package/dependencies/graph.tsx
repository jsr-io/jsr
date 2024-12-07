// Copyright 2024 the JSR authors. All rights reserved. MIT license.
import { HttpError, type RouteConfig } from "fresh";
import type { Dependency } from "../../../utils/api_types.ts";
import { path } from "../../../utils/api.ts";
import { scopeIAM } from "../../../utils/iam.ts";
import { define } from "../../../util.ts";
import {
  DependencyGraph,
  DependencyGraphProps,
} from "../(_islands)/DependencyGraph.tsx";
import { packageDataWithVersion } from "../../../utils/data.ts";
import { PackageHeader } from "../(_components)/PackageHeader.tsx";
import { PackageNav, type Params } from "../(_components)/PackageNav.tsx";

const dependencies = [
  {
    "dependency": {
      "type": "jsr",
      "scope": "std",
      "package": "assert",
      "version": "0.225.3",
      "path": "/assertion_error.ts",
    },
    "children": [],
    "size": 484,
    "mediaType": "TypeScript",
  },
  {
    "dependency": {
      "type": "jsr",
      "scope": "std",
      "package": "assert",
      "version": "0.225.3",
      "path": "/assert.ts",
    },
    "children": [
      0,
    ],
    "size": 562,
    "mediaType": "TypeScript",
  },
  {
    "dependency": {
      "type": "root",
      "path": "/cookie.ts",
    },
    "children": [
      1,
    ],
    "size": 11310,
    "mediaType": "TypeScript",
  },
  {
    "dependency": {
      "type": "jsr",
      "scope": "std",
      "package": "encoding",
      "version": "0.224.3",
      "path": "/_validate_binary_like.ts",
    },
    "children": [],
    "size": 798,
    "mediaType": "TypeScript",
  },
  {
    "dependency": {
      "type": "jsr",
      "scope": "std",
      "package": "encoding",
      "version": "0.224.3",
      "path": "/base64.ts",
    },
    "children": [
      3,
    ],
    "size": 3336,
    "mediaType": "TypeScript",
  },
  {
    "dependency": {
      "type": "root",
      "path": "/etag.ts",
    },
    "children": [
      4,
    ],
    "size": 6579,
    "mediaType": "TypeScript",
  },
  {
    "dependency": {
      "type": "root",
      "path": "/status.ts",
    },
    "children": [],
    "size": 13575,
    "mediaType": "TypeScript",
  },
  {
    "dependency": {
      "type": "root",
      "path": "/_negotiation/common.ts",
    },
    "children": [],
    "size": 1801,
    "mediaType": "TypeScript",
  },
  {
    "dependency": {
      "type": "root",
      "path": "/_negotiation/encoding.ts",
    },
    "children": [
      7,
    ],
    "size": 4301,
    "mediaType": "TypeScript",
  },
  {
    "dependency": {
      "type": "root",
      "path": "/_negotiation/language.ts",
    },
    "children": [
      7,
    ],
    "size": 4150,
    "mediaType": "TypeScript",
  },
  {
    "dependency": {
      "type": "root",
      "path": "/_negotiation/media_type.ts",
    },
    "children": [
      7,
    ],
    "size": 4970,
    "mediaType": "TypeScript",
  },
  {
    "dependency": {
      "type": "root",
      "path": "/negotiation.ts",
    },
    "children": [
      8,
      9,
      10,
    ],
    "size": 6414,
    "mediaType": "TypeScript",
  },
  {
    "dependency": {
      "type": "jsr",
      "scope": "std",
      "package": "async",
      "version": "0.224.1",
      "path": "/delay.ts",
    },
    "children": [],
    "size": 1895,
    "mediaType": "TypeScript",
  },
  {
    "dependency": {
      "type": "root",
      "path": "/server.ts",
    },
    "children": [
      12,
    ],
    "size": 25885,
    "mediaType": "TypeScript",
  },
  {
    "dependency": {
      "type": "jsr",
      "scope": "std",
      "package": "encoding",
      "version": "0.224.3",
      "path": "/hex.ts",
    },
    "children": [
      3,
    ],
    "size": 3097,
    "mediaType": "TypeScript",
  },
  {
    "dependency": {
      "type": "root",
      "path": "/unstable_signed_cookie.ts",
    },
    "children": [
      14,
    ],
    "size": 3687,
    "mediaType": "TypeScript",
  },
  {
    "dependency": {
      "type": "root",
      "path": "/server_sent_event_stream.ts",
    },
    "children": [],
    "size": 2761,
    "mediaType": "TypeScript",
  },
  {
    "dependency": {
      "type": "root",
      "path": "/user_agent.ts",
    },
    "children": [
      1,
    ],
    "size": 36299,
    "mediaType": "TypeScript",
  },
  {
    "dependency": {
      "type": "jsr",
      "scope": "std",
      "package": "path",
      "version": "0.225.1",
      "path": "/_common/assert_path.ts",
    },
    "children": [],
    "size": 307,
    "mediaType": "TypeScript",
  },
  {
    "dependency": {
      "type": "jsr",
      "scope": "std",
      "package": "path",
      "version": "0.225.1",
      "path": "/_common/normalize.ts",
    },
    "children": [
      18,
    ],
    "size": 263,
    "mediaType": "TypeScript",
  },
  {
    "dependency": {
      "type": "jsr",
      "scope": "std",
      "package": "path",
      "version": "0.225.1",
      "path": "/_common/constants.ts",
    },
    "children": [],
    "size": 2020,
    "mediaType": "TypeScript",
  },
  {
    "dependency": {
      "type": "jsr",
      "scope": "std",
      "package": "path",
      "version": "0.225.1",
      "path": "/_common/normalize_string.ts",
    },
    "children": [
      20,
    ],
    "size": 2301,
    "mediaType": "TypeScript",
  },
  {
    "dependency": {
      "type": "jsr",
      "scope": "std",
      "package": "path",
      "version": "0.225.1",
      "path": "/posix/_util.ts",
    },
    "children": [
      20,
    ],
    "size": 391,
    "mediaType": "TypeScript",
  },
  {
    "dependency": {
      "type": "jsr",
      "scope": "std",
      "package": "path",
      "version": "0.225.1",
      "path": "/posix/normalize.ts",
    },
    "children": [
      19,
      21,
      22,
    ],
    "size": 1056,
    "mediaType": "TypeScript",
  },
  {
    "dependency": {
      "type": "jsr",
      "scope": "std",
      "package": "path",
      "version": "0.225.1",
      "path": "/posix/join.ts",
    },
    "children": [
      18,
      23,
    ],
    "size": 721,
    "mediaType": "TypeScript",
  },
  {
    "dependency": {
      "type": "jsr",
      "scope": "std",
      "package": "path",
      "version": "0.225.1",
      "path": "/_os.ts",
    },
    "children": [],
    "size": 705,
    "mediaType": "TypeScript",
  },
  {
    "dependency": {
      "type": "jsr",
      "scope": "std",
      "package": "path",
      "version": "0.225.1",
      "path": "/posix/extname.ts",
    },
    "children": [
      20,
      18,
      22,
    ],
    "size": 2186,
    "mediaType": "TypeScript",
  },
  {
    "dependency": {
      "type": "jsr",
      "scope": "std",
      "package": "path",
      "version": "0.225.1",
      "path": "/windows/_util.ts",
    },
    "children": [
      20,
    ],
    "size": 828,
    "mediaType": "TypeScript",
  },
  {
    "dependency": {
      "type": "jsr",
      "scope": "std",
      "package": "path",
      "version": "0.225.1",
      "path": "/windows/extname.ts",
    },
    "children": [
      20,
      18,
      27,
    ],
    "size": 2342,
    "mediaType": "TypeScript",
  },
  {
    "dependency": {
      "type": "jsr",
      "scope": "std",
      "package": "path",
      "version": "0.225.1",
      "path": "/extname.ts",
    },
    "children": [
      25,
      26,
      28,
    ],
    "size": 547,
    "mediaType": "TypeScript",
  },
  {
    "dependency": {
      "type": "jsr",
      "scope": "std",
      "package": "path",
      "version": "0.225.1",
      "path": "/windows/normalize.ts",
    },
    "children": [
      19,
      20,
      21,
      27,
    ],
    "size": 3786,
    "mediaType": "TypeScript",
  },
  {
    "dependency": {
      "type": "jsr",
      "scope": "std",
      "package": "path",
      "version": "0.225.1",
      "path": "/windows/join.ts",
    },
    "children": [
      1,
      18,
      27,
      30,
    ],
    "size": 2483,
    "mediaType": "TypeScript",
  },
  {
    "dependency": {
      "type": "jsr",
      "scope": "std",
      "package": "path",
      "version": "0.225.1",
      "path": "/join.ts",
    },
    "children": [
      25,
      24,
      31,
    ],
    "size": 510,
    "mediaType": "TypeScript",
  },
  {
    "dependency": {
      "type": "jsr",
      "scope": "std",
      "package": "path",
      "version": "0.225.1",
      "path": "/posix/resolve.ts",
    },
    "children": [
      21,
      18,
      22,
    ],
    "size": 1586,
    "mediaType": "TypeScript",
  },
  {
    "dependency": {
      "type": "jsr",
      "scope": "std",
      "package": "path",
      "version": "0.225.1",
      "path": "/_common/relative.ts",
    },
    "children": [
      18,
    ],
    "size": 287,
    "mediaType": "TypeScript",
  },
  {
    "dependency": {
      "type": "jsr",
      "scope": "std",
      "package": "path",
      "version": "0.225.1",
      "path": "/posix/relative.ts",
    },
    "children": [
      22,
      33,
      34,
    ],
    "size": 3000,
    "mediaType": "TypeScript",
  },
  {
    "dependency": {
      "type": "jsr",
      "scope": "std",
      "package": "path",
      "version": "0.225.1",
      "path": "/windows/resolve.ts",
    },
    "children": [
      20,
      21,
      18,
      27,
    ],
    "size": 4848,
    "mediaType": "TypeScript",
  },
  {
    "dependency": {
      "type": "jsr",
      "scope": "std",
      "package": "path",
      "version": "0.225.1",
      "path": "/windows/relative.ts",
    },
    "children": [
      20,
      36,
      34,
    ],
    "size": 3978,
    "mediaType": "TypeScript",
  },
  {
    "dependency": {
      "type": "jsr",
      "scope": "std",
      "package": "path",
      "version": "0.225.1",
      "path": "/relative.ts",
    },
    "children": [
      25,
      35,
      37,
    ],
    "size": 788,
    "mediaType": "TypeScript",
  },
  {
    "dependency": {
      "type": "jsr",
      "scope": "std",
      "package": "path",
      "version": "0.225.1",
      "path": "/resolve.ts",
    },
    "children": [
      25,
      33,
      36,
    ],
    "size": 528,
    "mediaType": "TypeScript",
  },
  {
    "dependency": {
      "type": "jsr",
      "scope": "std",
      "package": "path",
      "version": "0.225.1",
      "path": "/constants.ts",
    },
    "children": [
      25,
    ],
    "size": 348,
    "mediaType": "TypeScript",
  },
  {
    "dependency": {
      "type": "jsr",
      "scope": "std",
      "package": "media-types",
      "version": "1.0.0-rc.1",
      "path": "/_util.ts",
    },
    "children": [],
    "size": 3253,
    "mediaType": "TypeScript",
  },
  {
    "dependency": {
      "type": "jsr",
      "scope": "std",
      "package": "media-types",
      "version": "1.0.0-rc.1",
      "path": "/parse_media_type.ts",
    },
    "children": [
      41,
    ],
    "size": 3636,
    "mediaType": "TypeScript",
  },
  {
    "dependency": {
      "type": "jsr",
      "scope": "std",
      "package": "media-types",
      "version": "1.0.0-rc.1",
      "path": "/vendor/mime-db.v1.52.0.ts",
    },
    "children": [],
    "size": 186498,
    "mediaType": "TypeScript",
  },
  {
    "dependency": {
      "type": "jsr",
      "scope": "std",
      "package": "media-types",
      "version": "1.0.0-rc.1",
      "path": "/_db.ts",
    },
    "children": [
      43,
      41,
    ],
    "size": 1347,
    "mediaType": "TypeScript",
  },
  {
    "dependency": {
      "type": "jsr",
      "scope": "std",
      "package": "media-types",
      "version": "1.0.0-rc.1",
      "path": "/get_charset.ts",
    },
    "children": [
      42,
      41,
      44,
    ],
    "size": 1497,
    "mediaType": "TypeScript",
  },
  {
    "dependency": {
      "type": "jsr",
      "scope": "std",
      "package": "media-types",
      "version": "1.0.0-rc.1",
      "path": "/format_media_type.ts",
    },
    "children": [
      41,
    ],
    "size": 2539,
    "mediaType": "TypeScript",
  },
  {
    "dependency": {
      "type": "jsr",
      "scope": "std",
      "package": "media-types",
      "version": "1.0.0-rc.1",
      "path": "/type_by_extension.ts",
    },
    "children": [
      44,
    ],
    "size": 1203,
    "mediaType": "TypeScript",
  },
  {
    "dependency": {
      "type": "jsr",
      "scope": "std",
      "package": "media-types",
      "version": "1.0.0-rc.1",
      "path": "/content_type.ts",
    },
    "children": [
      42,
      45,
      46,
      44,
      47,
    ],
    "size": 3552,
    "mediaType": "TypeScript",
  },
  {
    "dependency": {
      "type": "jsr",
      "scope": "std",
      "package": "streams",
      "version": "0.224.2",
      "path": "/byte_slice_stream.ts",
    },
    "children": [
      1,
    ],
    "size": 2657,
    "mediaType": "TypeScript",
  },
  {
    "dependency": {
      "type": "jsr",
      "scope": "std",
      "package": "cli",
      "version": "0.224.4",
      "path": "/parse_args.ts",
    },
    "children": [
      1,
    ],
    "size": 22373,
    "mediaType": "TypeScript",
  },
  {
    "dependency": {
      "type": "jsr",
      "scope": "std",
      "package": "fmt",
      "version": "0.225.2",
      "path": "/colors.ts",
    },
    "children": [],
    "size": 21644,
    "mediaType": "TypeScript",
  },
  {
    "dependency": {
      "type": "root",
      "path": "/deno.json",
    },
    "children": [],
    "size": 461,
    "mediaType": "Json",
  },
  {
    "dependency": {
      "type": "jsr",
      "scope": "std",
      "package": "fmt",
      "version": "0.225.2",
      "path": "/bytes.ts",
    },
    "children": [],
    "size": 4665,
    "mediaType": "TypeScript",
  },
  {
    "dependency": {
      "type": "root",
      "path": "/file_server.ts",
    },
    "children": [
      24,
      23,
      29,
      32,
      38,
      39,
      40,
      48,
      5,
      6,
      49,
      50,
      51,
      52,
      53,
    ],
    "size": 25534,
    "mediaType": "TypeScript",
  },
  {
    "dependency": {
      "type": "root",
      "path": "/mod.ts",
    },
    "children": [
      2,
      5,
      6,
      11,
      13,
      15,
      16,
      17,
      54,
    ],
    "size": 2380,
    "mediaType": "TypeScript",
  },
] as const satisfies DependencyGraphProps["dependencies"];

export default define.page<typeof handler>(
  function DepsGraph({ data, params, state }) {
    const iam = scopeIAM(state, data.member);

    return (
      <div class="mb-20">
        <PackageHeader
          package={data.package}
          selectedVersion={data.selectedVersion}
        />

        <PackageNav
          currentTab="Dependencies"
          versionCount={data.package.versionCount}
          iam={iam}
          params={params as unknown as Params}
          latestVersion={data.package.latestVersion}
        />

        <div class="space-y-3 mt-8">
          <DependencyGraph dependencies={dependencies} />
        </div>
      </div>
    );
  },
);

export const handler = define.handlers({
  async GET(ctx) {
    const res = await packageDataWithVersion(
      ctx.state,
      ctx.params.scope,
      ctx.params.package,
      ctx.params.version,
    );
    if (res === null) {
      throw new HttpError(
        404,
        "This package or this package version was not found.",
      );
    }

    const {
      pkg,
      scopeMember,
      selectedVersion,
    } = res;

    if (selectedVersion === null) {
      return new Response(null, {
        status: 302,
        headers: {
          Location: `/@${ctx.params.scope}/${ctx.params.package}`,
        },
      });
    }

    const depsResp = await ctx.state.api.get<Dependency[]>(
      path`/scopes/${pkg.scope}/packages/${pkg.name}/versions/${selectedVersion.version}/dependencies`,
    );
    if (!depsResp.ok) throw depsResp;

    ctx.state.meta = {
      title: `Dependencies - @${pkg.scope}/${pkg.name} - JSR`,
      description: `@${pkg.scope}/${pkg.name} on JSR${
        pkg.description ? `: ${pkg.description}` : ""
      }`,
    };

    return {
      data: {
        package: pkg,
        deps: depsResp.data,
        selectedVersion,
        member: scopeMember,
      },
      headers: { "X-Robots-Tag": "noindex" },
    };
  },
});

export const config: RouteConfig = {
  routeOverride: "/@:scope/:package{@:version}?/dependencies/graph",
};
