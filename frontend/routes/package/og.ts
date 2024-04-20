import { Handlers, RouteConfig } from "$fresh/server.ts";
import { Image } from "$imagescript";

import { packageDataWithVersion } from "../../utils/data.ts";
import { State } from "../../util.ts";
import twas from "$twas";
import { getScoreTextColorClass } from "../../utils/score_ring_color.ts";
import { RUNTIME_COMPAT_KEYS } from "../../components/RuntimeCompatIndicator.tsx";

const SCORE_CLASSNAME_TO_COLOR_MAP: Record<string, number> = {
  "score-text-green": Image.rgbToColor(34, 197, 94),
  "score-text-yellow": Image.rgbToColor(234, 178, 8),
  "score-text-red": Image.rgbToColor(239, 68, 68),
};

const jsrLogo = await Image.decode(await Deno.readFile("./static/logo.png"));

let dmmonoFont: Uint8Array | null = null;

export const handler: Handlers<undefined, State> = {
  async GET(_req, ctx) {
    if (!dmmonoFont) {
      dmmonoFont = await Deno.readFile(
        "./static/fonts/DMMono/DMMono-Medium.ttf",
      );
    }
    const pkgData = await packageDataWithVersion(
      ctx.state,
      ctx.params.scope,
      ctx.params.package,
      ctx.params.version,
    );
    if (!pkgData) {
      return ctx.renderNotFound();
    }
    const { pkg, selectedVersion } = pkgData;

    if (!selectedVersion) {
      return ctx.renderNotFound();
    }

    const ogpImage = new Image(1200, 630);

    // Generate header
    const headerBaseX = 30;
    const headerBaseY = 30;
    const packageNameText = Image.renderText(
      dmmonoFont,
      50,
      `@${pkg.scope}/${pkg.name}`,
      Image.rgbToColor(0, 0, 0),
    );
    ogpImage.composite(packageNameText, headerBaseX, headerBaseY);
    const versionText = Image.renderText(
      dmmonoFont,
      30,
      `@${selectedVersion.version}`,
      Image.rgbToColor(50, 50, 50),
    );
    ogpImage.composite(
      versionText,
      headerBaseX + packageNameText.width + 20,
      headerBaseY + packageNameText.height / 4,
    );

    if (selectedVersion.version === pkg.latestVersion) {
      const badgeColor = Image.rgbToColor(247, 222, 30);
      const latestText = Image.renderText(
        dmmonoFont,
        20,
        "latest",
        Image.rgbToColor(0, 0, 0),
      );

      const badgeWidth = 100;
      const badgeHeight = 40;

      const latestBadge = new Image(badgeWidth, badgeHeight)
        .drawCircle(
          badgeHeight / 2,
          badgeHeight / 2 + 1,
          badgeHeight / 2,
          badgeColor,
        )
        .drawBox(
          badgeHeight / 2,
          0,
          badgeWidth - badgeHeight,
          badgeHeight,
          badgeColor,
        )
        .drawCircle(
          badgeWidth - badgeHeight / 2,
          badgeHeight / 2 + 1,
          badgeHeight / 2,
          badgeColor,
        )
        .composite(
          latestText,
          (badgeWidth - latestText.width) / 2,
          badgeHeight / 4,
        );
      ogpImage.composite(
        latestBadge,
        headerBaseX + packageNameText.width + versionText.width + 40,
        headerBaseY + 14,
      );
    }
    const descriptionText = Image.renderText(
      dmmonoFont,
      32,
      (pkg.description.length > 50
        ? pkg.description.slice(0, 50) + "..."
        : pkg.description) || "No description",
      Image.rgbToColor(50, 50, 50),
    );
    ogpImage.composite(
      descriptionText,
      headerBaseX + 16,
      headerBaseY + packageNameText.height + 16,
    );

    // Package Infomations such as Runtime compats, JSR Score and Published
    const packageInfomationDefaultY = 300;

    // Published
    const publishedText = Image.renderText(
      dmmonoFont,
      32,
      "Published",
      Image.rgbToColor(0, 0, 0),
    );
    ogpImage.composite(publishedText, 50, packageInfomationDefaultY)
      .composite(
        Image.renderText(
          dmmonoFont,
          25,
          twas(new Date(selectedVersion.createdAt)),
          Image.rgbToColor(50, 50, 50),
        ),
        60,
        publishedText.height + packageInfomationDefaultY,
      );

    // JSR Score
    const jsrScoreLabel = Image.renderText(
      dmmonoFont,
      32,
      "JSR Score",
      Image.rgbToColor(0, 0, 0),
    );
    const scoreColor =
      SCORE_CLASSNAME_TO_COLOR_MAP[getScoreTextColorClass(pkg.score ?? 0)];
    ogpImage.composite(
      jsrScoreLabel,
      600 - jsrScoreLabel.width / 2,
      packageInfomationDefaultY,
    );
    const jsrScore = Image.renderText(
      dmmonoFont,
      60,
      `${pkg.score}%`,
      scoreColor,
    );
    ogpImage.composite(
      jsrScore,
      600 - jsrScore.width / 2,
      packageInfomationDefaultY + jsrScoreLabel.height,
    );

    // Runtime compats
    const runtimeCompatsText = Image.renderText(
      dmmonoFont,
      32,
      "Works with",
      Image.rgbToColor(0, 0, 0),
    );
    ogpImage.composite(
      runtimeCompatsText,
      1100 - runtimeCompatsText.width,
      packageInfomationDefaultY,
    );
    const questionMark = Image.renderText(
      dmmonoFont,
      50,
      "?",
      Image.rgbToColor(29, 78, 216),
    );
    let runtimeKeyWidth = 0;
    for (const runtimeKey of RUNTIME_COMPAT_KEYS.toReversed()) {
      const [key, _name, icon, width, height] = runtimeKey;
      const compat = pkg.runtimeCompat[key];
      if (compat === false) {
        // Not supported
        continue;
      }
      const iconData = await Deno.readTextFile(`./static${icon}`);
      const iconImage = Image.renderSVG(iconData, 50 / height);

      const supportedIcon = compat
        ? iconImage
        : iconImage.saturation(0).opacity(0.4).composite(
          questionMark,
          (width / 2) * 50 / height - questionMark.width / 2,
        );
      runtimeKeyWidth += supportedIcon.width;

      ogpImage.composite(
        supportedIcon,
        1100 - runtimeKeyWidth,
        packageInfomationDefaultY + runtimeCompatsText.height + 16,
      );
    }

    // JSR Brand
    const logoWidth = jsrLogo.width * 100 / jsrLogo.height;
    ogpImage.composite(jsrLogo.resize(logoWidth, 100), 1100 - logoWidth, 500);

    return new Response(await ogpImage.encode());
  },
};

export const config: RouteConfig = {
  routeOverride: "/@:scope/:package{@:version}?/og",
};
