// Copyright 2024 the JSR authors. All rights reserved. MIT license.
import type { Handlers, RouteConfig } from "$fresh/server.ts";
import { accepts } from "$oak_commons";
import { Package } from "../utils/api_types.ts";
import { path } from "../utils/api.ts";
import { State } from "../util.ts";

export const handler: Handlers<unknown, State> = {
  async GET(req, ctx) {
    if (
      accepts(req, "application/json", "text/html", "image/*") ===
        "application/json"
    ) {
      const packageResp = await ctx.state.api.get<Package>(
        path`/scopes/${ctx.params.scope}/packages/${ctx.params.package}`,
      );

      if (!packageResp.ok) {
        if (packageResp.code === "packageNotFound") {
          return new Response(null, { status: 404 });
        } else {
          throw packageResp;
        }
      } else {
        return new Response(
          JSON.stringify({
            schemaVersion: 1,
            label: "",
            // namedLogo: "jsr", TODO: add icon to shields.io or simple-icons. temporary solution below.
            logoSvg:
              '<svg xmlns="http://www.w3.org/2000/svg" width="638" height="638" fill="none"><g fill-rule="evenodd"><path fill="#121417" d="M637.272 196v196h-98v98h-343v-49h-196V245h98v-98h343v49h196Z"/><path fill="#F7DF1E" d="M100.101 343h47.171V196h49v196H51.102v-98H100.1v49ZM588.272 245v98h-49v-49h-49v147h-49V245h147ZM294.272 245v49h98v147h-147v-49h98v-49h-98V196h147v49h-98Z"/></g></svg>',
            message: packageResp.data.latestVersion,
            labelColor: "#121417",
            color: "#F7DF1E",
          }),
          {
            headers: {
              "content-type": "application/json",
            },
          },
        );
      }
    } else {
      const url = new URL(req.url);
      url.protocol = "https:";

      const shieldsUrl = new URL("https://img.shields.io/endpoint");
      shieldsUrl.search = url.search;
      shieldsUrl.searchParams.set("url", url.href);

      const res = await fetch(shieldsUrl);

      return new Response(res.body, {
        status: res.status,
        headers: {
          "content-type": res.headers.get("content-type")!,
        },
      });
    }
  },
};

export const config: RouteConfig = {
  routeOverride: "/badges/@:scope/:package",
};
