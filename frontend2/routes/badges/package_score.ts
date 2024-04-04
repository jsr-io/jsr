// Copyright 2024 the JSR authors. All rights reserved. MIT license.
import type { Handlers, RouteConfig } from "$fresh";
import { accepts } from "$oak_commons";
import { Package } from "../../utils/api_types.ts";
import { path } from "../../utils/api.ts";
import { State } from "../../util.ts";

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
        if (packageResp.data.score === null) {
          return new Response(null, { status: 404 });
        }

        return new Response(
          JSON.stringify({
            schemaVersion: 1,
            label: "",
            // namedLogo: "jsr", TODO: add icon to shields.io or simple-icons. temporary solution below.
            logoSvg: await Deno.readTextFile(
              new URL("../../static/logo.svg", import.meta.url),
            ),
            message: `${packageResp.data.score}%`,
            labelColor: "rgb(8,51,68)",
            color: "rgb(247,223,30)",
            logoWidth: "25",
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
  routeOverride: "/badges/@:scope/:package/score",
};
