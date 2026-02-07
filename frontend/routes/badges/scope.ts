// Copyright 2024 the JSR authors. All rights reserved. MIT license.
import type { RouteConfig } from "fresh";
import { accepts } from "@std/http/negotiation";
import { define } from "../../util.ts";
import { Scope } from "../../utils/api_types.ts";
import { assertOk, path } from "../../utils/api.ts";
import { primaryColor, secondaryColor } from "../../utils/colors.ts";

export const handler = define.handlers({
  async GET(ctx) {
    const req = ctx.req;

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
          assertOk(scopeResp);
        }
      } else {
        return Response.json({
          schemaVersion: 1,
          label: "",
          message: `@${scopeResp.data.scope}`,
          labelColor: secondaryColor,
          color: primaryColor,
        });
      }
    } else {
      const url = new URL("https://jsr.io" + ctx.url.pathname + ctx.url.search);

      const shieldsUrl = new URL("https://img.shields.io/endpoint");
      shieldsUrl.search = url.search;
      shieldsUrl.searchParams.set("url", url.href);
      shieldsUrl.searchParams.set("logo", "jsr");
      shieldsUrl.searchParams.set("logoSize", "auto");
      shieldsUrl.searchParams.set("cacheSeconds", "300");

      if (!ctx.url.searchParams.has("logoColor")) {
        shieldsUrl.searchParams.set("logoColor", primaryColor);
      }

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
});

export const config: RouteConfig = {
  routeOverride: "/badges/@:scope",
};
