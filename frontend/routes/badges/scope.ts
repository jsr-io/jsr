// Copyright 2024 the JSR authors. All rights reserved. MIT license.
import type { Handlers, RouteConfig } from "$fresh/server.ts";
import { accepts } from "$oak_commons";
import { Scope } from "../../utils/api_types.ts";
import { path } from "../../utils/api.ts";
import { State } from "../../util.ts";

export const handler: Handlers<unknown, State> = {
  async GET(req, ctx) {
    if (
      accepts(req, "application/json", "text/html", "image/*") ===
        "application/json"
    ) {
      const scopeResp = await ctx.state.api.get<Scope>(
        path`/scopes/${ctx.params.scope}`,
      );

      if (!scopeResp.ok) {
        if (scopeResp.code === "scopeNotFound") {
          return new Response(null, { status: 404 });
        } else {
          throw scopeResp;
        }
      } else {
        return Response.json({
          schemaVersion: 1,
          label: "",
          message: `@${scopeResp.data.scope}`,
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

      const res = await fetch(shieldsUrl);

      return new Response(res.body, {
        status: res.status,
        headers: {
          "cache-control": "max-age=300, s-maxage=300",
          "content-type": res.headers.get("content-type")!,
        },
      });
    }
  },
};

export const config: RouteConfig = {
  routeOverride: "/badges/@:scope",
};
