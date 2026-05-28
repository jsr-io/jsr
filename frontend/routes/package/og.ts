// Copyright 2024 the JSR authors. All rights reserved. MIT license.

import { HttpError, RouteConfig } from "fresh";
import { ImageResponse } from "workers-og";
import twas from "twas";

import { packageDataWithVersion } from "../../utils/data.ts";
import { define } from "../../util.ts";
import { getScoreTextColorClass } from "../../utils/score_ring_color.ts";
import { RUNTIME_COMPAT_KEYS } from "../../components/RuntimeCompatIndicator.tsx";
import { readAsset, readAssetText } from "../../utils/assets.ts";

const WIDTH = 1200;
const HEIGHT = 630;
const PADDING = 30;

const SCORE_HEX: Record<string, string> = {
  "score-text-green": "#22c55e",
  "score-text-yellow": "#eab208",
  "score-text-red": "#ef4444",
};

let dmmonoFont: Promise<Uint8Array> | null = null;
function getFont(): Promise<Uint8Array> {
  return dmmonoFont ??= readAsset("/fonts/DMMono/DMMono-Medium.ttf");
}

let jsrLogoDataUrl: Promise<string> | null = null;
function getJsrLogoDataUrl(): Promise<string> {
  return jsrLogoDataUrl ??= (async () => {
    const bytes = await readAsset("/logo.png");
    let bin = "";
    for (const b of bytes) bin += String.fromCharCode(b);
    return `data:image/png;base64,${btoa(bin)}`;
  })();
}

function escape(s: string): string {
  return s.replace(/&/g, "&amp;").replace(/</g, "&lt;").replace(/>/g, "&gt;")
    .replace(/"/g, "&quot;");
}

async function renderRuntimeIcons(
  // deno-lint-ignore no-explicit-any
  compat: Record<string, any>,
): Promise<string> {
  const cells: string[] = [];
  for (
    const [key, _name, icon, width, height] of RUNTIME_COMPAT_KEYS.toReversed()
  ) {
    const c = compat[key];
    if (c === false) continue;
    const svg = await readAssetText(icon);
    const dataUrl = `data:image/svg+xml;base64,${
      btoa(unescape(encodeURIComponent(svg)))
    }`;
    const scaledW = (width / height) * 50;
    const opacity = c ? 1 : 0.4;
    cells.push(
      `<img src="${dataUrl}" width="${scaledW}" height="50" style="opacity:${opacity};margin-right:8px;" />`,
    );
  }
  return cells.join("");
}

export const handler = define.handlers({
  async GET(ctx) {
    const pkgData = await packageDataWithVersion(
      ctx.state,
      ctx.params.scope,
      ctx.params.package,
      ctx.params.version,
    );
    if (!pkgData || !pkgData.selectedVersion) {
      throw new HttpError(
        404,
        "This package or this package version was not found.",
      );
    }
    const { pkg, selectedVersion } = pkgData;

    const isLatest = selectedVersion.version === pkg.latestVersion;
    const scoreColor = SCORE_HEX[getScoreTextColorClass(pkg.score ?? 0)] ??
      "#000";

    const [fontData, logoUrl, runtimeIconsHtml] = await Promise.all([
      getFont(),
      getJsrLogoDataUrl(),
      renderRuntimeIcons(pkg.runtimeCompat),
    ]);

    const description = pkg.description?.trim() || "No description";
    const publishedRel = twas(new Date(selectedVersion.createdAt).getTime());

    const latestBadge = isLatest
      ? `<div style="display:flex;align-items:center;background:#f7de1e;color:#000;border-radius:9999px;padding:4px 18px;margin-left:14px;font-size:20px;">latest</div>`
      : "";

    const html = `
      <div style="display:flex;flex-direction:column;width:${WIDTH}px;height:${HEIGHT}px;background:#fff;padding:${PADDING}px;font-family:DMMono;color:#000;">
        <div style="display:flex;justify-content:space-between;align-items:flex-start;">
          <div style="display:flex;font-size:50px;line-height:1;flex-wrap:wrap;max-width:900px;">
            <span style="color:#464646;">@${escape(pkg.scope)}</span><span>/${
      escape(pkg.name)
    }</span>
          </div>
          <div style="display:flex;align-items:center;font-size:30px;color:#323232;">
            @${escape(selectedVersion.version)}${latestBadge}
          </div>
        </div>
        <div style="display:flex;font-size:30px;color:#464646;margin-top:36px;flex:1;">${
      escape(description)
    }</div>
        <div style="display:flex;justify-content:space-between;align-items:flex-end;margin-top:24px;">
          <div style="display:flex;flex-direction:column;">
            <div style="font-size:32px;color:#000;">Published</div>
            <div style="font-size:25px;color:#464646;margin-top:10px;">${
      escape(publishedRel)
    }</div>
          </div>
          <div style="display:flex;flex-direction:column;">
            <div style="font-size:32px;color:#000;">JSR Score</div>
            <div style="font-size:60px;color:${scoreColor};line-height:1;margin-top:6px;">${
      escape(String(pkg.score ?? 0))
    }%</div>
          </div>
          <div style="display:flex;flex-direction:column;">
            <div style="font-size:32px;color:#000;">Works with</div>
            <div style="display:flex;margin-top:10px;">${runtimeIconsHtml}</div>
          </div>
        </div>
        <img src="${logoUrl}" height="80" style="position:absolute;right:${PADDING}px;bottom:${PADDING}px;" />
      </div>
    `;

    ctx.state.cacheControl =
      "public, max-age=60, s-maxage=86400, stale-while-revalidate=86400";

    return new ImageResponse(html, {
      width: WIDTH,
      height: HEIGHT,
      fonts: [
        {
          name: "DMMono",
          data: fontData.buffer as ArrayBuffer,
          weight: 500,
          style: "normal",
        },
      ],
    });
  },
});

export const config: RouteConfig = {
  routeOverride: "/@:scope/:package{@:version}?/og",
};
