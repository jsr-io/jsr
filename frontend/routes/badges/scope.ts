// Copyright 2024 the JSR authors. All rights reserved. MIT license.
import type { RouteConfig } from "fresh";
import { accepts } from "@std/http/negotiation";
import { define } from "../../util.ts";
import { Scope } from "../../utils/api_types.ts";
import { path } from "../../utils/api.ts";
import { makeBadge } from "badge-maker";

export const handler = define.handlers({
  async GET(ctx) {
    const req = ctx.req;

    const scopeResp = await ctx.state.api.get<Scope>(
      path`/scopes/${ctx.params.scope}`,
    );

    if (
      accepts(req, "application/json", "text/html", "image/*") ===
        "application/json"
    ) {
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
      let badge: string;

      if (!scopeResp.ok) {
        if (scopeResp.code === "scopeNotFound") {
          badge = makeBadge({
            label: "custom badge",
            message: "resource not found",
            color: "rgb(206,88,66)",
          });
        } else {
          throw scopeResp;
        }
      } else {
        const url = new URL(
          "https://jsr.io" + ctx.url.pathname + ctx.url.search,
        );
        badge = makeBadge({
          label: "",
          message: `@${scopeResp.data.scope}`,
          labelColor: "rgb(247,223,30)",
          color: "rgb(8,51,68)",
          links: [url.toString()],
        });
      }

      return new Response(badge, {
        headers: {
          "access-control-allow-origin": "*",
          "cache-control": "max-age=300,s-maxage=300",
          "content-type": "image/svg+xml;charset=utf-8",
        },
      });
    }
  },
});

export const config: RouteConfig = {
  routeOverride: "/badges/@:scope",
};
