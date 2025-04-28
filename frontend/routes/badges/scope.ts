// Copyright 2024 the JSR authors. All rights reserved. MIT license.
import type { RouteConfig } from "fresh";
import { accepts } from "@std/http/negotiation";
import { define } from "../../util.ts";
import { Scope } from "../../utils/api_types.ts";
import { path } from "../../utils/api.ts";
import { makeBadge } from "badge-maker";

const SVG_LOGO = `\
<svg fill="rgb(8,51,68)" role="img" viewBox="0 0 24 12.924000000000003" xmlns="http://www.w3.org/2000/svg">
<title>JSR</title>
<path d="M3.692 0v3.693H0v7.384h7.385v1.847h12.923v-3.693H24V1.847h-7.385V0Zm1.846 1.847h1.847v7.384H1.846v-3.692h1.846v1.846h1.846z m3.693 0h5.538V3.692h-3.692v1.846h3.692v5.538H9.231V9.232h3.692v-1.846H9.231Zm7.384 1.846h5.539v3.692h-1.846v-1.846h-1.846v5.538h-1.847z"/>
</svg>
`;

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
	  logoBase64: `data:image/svg+xml,${SVG_LOGO}`
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
