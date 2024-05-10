// Copyright 2024 the JSR authors. All rights reserved. MIT license.
import type { Handlers, RouteConfig } from "$fresh/server.ts";
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
        return Response.json({
          schemaVersion: 1,
          label: "",
          message: packageResp.data.latestVersion,
          labelColor: "rgb(247,223,30)",
          color: "rgb(8,51,68)",
        });
      }
    } else {
      const url = new URL(req.url);
      url.protocol = "https:";

      const shieldsUrl = new URL("https://img.shields.io/endpoint");
      shieldsUrl.search = url.search;
      shieldsUrl.searchParams.set("url", url.href);
      shieldsUrl.searchParams.set("logo", "jsr");
      shieldsUrl.searchParams.set("logoColor", "rgb(8,51,68)");
      shieldsUrl.searchParams.set("logoSize", "auto");
      shieldsUrl.searchParams.set("cacheSeconds", "300");

      const res = await fetch(shieldsUrl);

      return new Response(res.body, {
        status: res.status,
        headers: {
          "access-control-allow-origin": res.headers.get(
            "access-control-allow-origin",
          )!,
          "cache-control": res.headers.get("cache-control")!,
          "content-type": res.headers.get("content-type")!,
        },
      });
    }
  },
};

export const config: RouteConfig = {
  routeOverride: "/badges/@:scope/:package",
};
