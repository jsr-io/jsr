#!/usr/bin/env -S deno run -A --watch
// Copyright 2024 the JSR authors. All rights reserved. MIT license.

import { isCDNRequest, proxy } from "./server.ts";

const FRONTEND_SERVER = "http://localhost:8000";
const API_SERVER = "https://api.jsr.io";
const CDN_SERVER = "https://jsr.io";

const DOMAIN = "jsr.test";

const PORT = 80;

async function handler(req: Request): Promise<Response> {
  const url = new URL(req.url);

  function redirectRoot() {
    url.hostname = DOMAIN;
    url.port = String(PORT);
    return Response.redirect(url.href, 307);
  }

  switch (url.hostname) {
    case DOMAIN: {
      if (isCDNRequest(req, url)) {
        const res = await fetch(
          `${CDN_SERVER}${url.pathname}`,
          { redirect: "manual", method: req.method, headers: req.headers },
        );
        return res;
      }
      if (url.pathname.startsWith("/api")) {
        const pathname = url.pathname.replace(/^\/api/, "");
        const apiUrl = `${API_SERVER}${pathname}${url.search}`;
        const apiRes = await proxy(req, apiUrl);
        return apiRes;
      }
      const frontendUrl = `${FRONTEND_SERVER}${url.pathname}${url.search}`;
      const frontendRes = await proxy(req, frontendUrl);
      return frontendRes;
    }
    default:
      return redirectRoot();
  }
}

Deno.serve({ port: PORT, hostname: "0.0.0.0" }, handler);
