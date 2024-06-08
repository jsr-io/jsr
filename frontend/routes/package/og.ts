// Copyright 2024 the JSR authors. All rights reserved. MIT license.

import { Handlers, RouteConfig } from "$fresh/server.ts";
import { Image } from "$imagescript";

import { packageDataWithVersion } from "../../utils/data.ts";
import { State } from "../../util.ts";
import twas from "$twas";
import { getScoreTextColorClass } from "../../utils/score_ring_color.ts";
import { RUNTIME_COMPAT_KEYS } from "../../components/RuntimeCompatIndicator.tsx";
import { PackageName } from "../../islands/new.tsx";

const SCORE_CLASSNAME_TO_COLOR_MAP: Record<string, number> = {
  "score-text-green": Image.rgbToColor(34, 197, 94),
  "score-text-yellow": Image.rgbToColor(234, 178, 8),
  "score-text-red": Image.rgbToColor(239, 68, 68),
};

const jsrLogo = await Image.decode(await Deno.readFile("./static/logo.png"));

let dmmonoFont: Uint8Array | null = null;

const COLOR_BLACK = Image.rgbToColor(0, 0, 0);
const COLOR_WHITE = Image.rgbToColor(255, 255, 255);
const COLOR_GRAY = Image.rgbToColor(70, 70, 70);

const PADDING = 30;

const WIDTH = 1200;
const HEIGHT = 630;

const LATEST_BADGE_WIDTH = 100;
const LATEST_BADGE_HEIGHT = 40;
const LATEST_BADGE_COLOR = Image.rgbToColor(247, 222, 30);

const JSR_LOGO_HEIGHT = 100;

const DESCRIPTION_MAX_BREAK_POINT = 60;

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

    const packageScope = pkg.scope;
    const packageName = pkg.name;

    const ogpImage = new Image(WIDTH, HEIGHT).drawBox(
      0,
      0,
      WIDTH,
      HEIGHT,
      COLOR_WHITE,
    );

    let packageNamePosition: {
      x: number;
      y: number;
      height: number;
      isSplited: boolean;
    };
    if (packageScope.length + packageName.length > 23) {
      // new line  | @package
      // example:  |  /name
      const scopeText = Image.renderText(
        dmmonoFont,
        50,
        `@${packageScope}`,
        COLOR_GRAY,
      );
      ogpImage.composite(scopeText, PADDING, PADDING);

      const packageNameText = Image.renderText(
        dmmonoFont,
        50,
        `/${packageName}`,
        COLOR_BLACK,
      );
      ogpImage.composite(packageNameText, PADDING, scopeText.height + 20);

      packageNamePosition = {
        x: PADDING + packageNameText.width,
        y: PADDING + scopeText.height + 20 + packageNameText.height,
        height: packageNameText.height,
        isSplited: true
      };
    } else {
      // one line
      // example: @package/name
      const scopeText = Image.renderText(
        dmmonoFont,
        50,
        `@${packageScope}`,
        COLOR_GRAY,
      );
      ogpImage.composite(scopeText, PADDING, PADDING);

      const packageNameText = Image.renderText(
        dmmonoFont,
        50,
        `/${packageName}`,
        COLOR_BLACK,
      );
      ogpImage.composite(
        packageNameText,
        PADDING + scopeText.width + 10,
        PADDING,
      );

      packageNamePosition = {
        x: PADDING + scopeText.width + 10 + packageNameText.width,
        y: PADDING + packageNameText.height,
        height: packageNameText.height,
        isSplited: false
      };
    }
    const isLatest = selectedVersion.version === pkg.latestVersion;

    const versionText = Image.renderText(
      dmmonoFont,
      30,
      `@${selectedVersion.version}`,
      Image.rgbToColor(50, 50, 50),
    );
    const versionAndLatestBadgeImage = new Image(
      versionText.width + (isLatest ? LATEST_BADGE_WIDTH + 10 : 0),
      Math.max(versionText.height, LATEST_BADGE_HEIGHT),
    );
    versionAndLatestBadgeImage.composite(versionText, 0, 0);

    if (isLatest) {
      // This version is latest
      const latestText = Image.renderText(
        dmmonoFont,
        20,
        "latest",
        COLOR_BLACK,
      );
      const latestBadge = new Image(LATEST_BADGE_WIDTH, LATEST_BADGE_HEIGHT)
        .drawCircle(
          LATEST_BADGE_HEIGHT / 2,
          LATEST_BADGE_HEIGHT / 2 + 1,
          LATEST_BADGE_HEIGHT / 2,
          LATEST_BADGE_COLOR,
        ).drawBox(
          LATEST_BADGE_HEIGHT / 2,
          0,
          LATEST_BADGE_WIDTH - LATEST_BADGE_HEIGHT,
          LATEST_BADGE_HEIGHT,
          LATEST_BADGE_COLOR,
        ).drawCircle(
          LATEST_BADGE_WIDTH - LATEST_BADGE_HEIGHT / 2,
          LATEST_BADGE_HEIGHT / 2 + 1,
          LATEST_BADGE_HEIGHT / 2,
          LATEST_BADGE_COLOR,
        ).composite(
          latestText,
          (LATEST_BADGE_WIDTH - latestText.width) / 2,
          (LATEST_BADGE_HEIGHT - latestText.height) / 2,
        );
      versionAndLatestBadgeImage.composite(
        latestBadge,
        versionText.width + 10,
        0,
      );
    }

    let descriptionY: number;
    const isVersionAndLatestBadgeNextLine = packageNamePosition.x > 900;
    if (isVersionAndLatestBadgeNextLine) {
      // Version/Latest will be new line
      const yPos = packageNamePosition.y;
      ogpImage.composite(
        versionAndLatestBadgeImage,
        WIDTH - PADDING - versionAndLatestBadgeImage.width,
        yPos,
      );
      descriptionY = yPos;
    } else {
      // Version/Latest will be current line
      const versionAndLatestBadgeYPadding = (packageNamePosition.height - versionAndLatestBadgeImage.height) / 2
      ogpImage.composite(
        versionAndLatestBadgeImage,
        packageNamePosition.x + 10,
        packageNamePosition.y - packageNamePosition.height + (packageNamePosition.isSplited ? -versionAndLatestBadgeYPadding : versionAndLatestBadgeYPadding)
      );
      descriptionY = packageNamePosition.y + 10;
    }
    const descriptionBreakPoint = isVersionAndLatestBadgeNextLine
      ? 45
      : DESCRIPTION_MAX_BREAK_POINT;

    const descriptionString: string = (() => {
      if (pkg.description.length <= descriptionBreakPoint) {
        // Don't cut
        return pkg.description;
      }
      const firstLine: string[] = [];

      const splittedBySpace = pkg.description.split(/(?<=\,?) /g);

      for (const word of splittedBySpace) {
        firstLine.push(word);
        const { width } = Image.renderText(dmmonoFont, 30, firstLine.join(" "));
        if (width > WIDTH - 2 * PADDING) {
          firstLine.pop();
          break;
        }
      }
      const firstLineText = firstLine.join(" ");
      const secondLine = pkg.description.slice(firstLineText.length).replace(
        /^ +/,
        "",
      );

      return firstLineText + "\n" +
        (secondLine.length >= DESCRIPTION_MAX_BREAK_POINT
          ? secondLine.slice(0, DESCRIPTION_MAX_BREAK_POINT - 3) + "..."
          : secondLine);
    })() || "No description";

    const descriptionText = Image.renderText(
      dmmonoFont,
      30,
      descriptionString,
      COLOR_GRAY,
    );
    ogpImage.composite(
      descriptionText,
      PADDING,
      descriptionY,
    );

    // Package Infomations such as Runtime compats, JSR Score and Published

    // Published
    const publishedImage = (() => {
      const publishedText = Image.renderText(
        dmmonoFont,
        32,
        "Published",
        COLOR_BLACK,
      );
      const publishDateText = Image.renderText(
        dmmonoFont,
        25,
        twas(new Date(selectedVersion.createdAt)),
        COLOR_GRAY,
      );
      const result = new Image(
        Math.max(publishedText.width, publishDateText.width),
        publishedText.height + 10 + publishDateText.height,
      );
      result.composite(publishedText, 0, 0);
      result.composite(publishDateText, 0, publishedText.height + 10);

      return result;
    })();

    // JSR Score
    const jsrScore = (() => {
      const jsrScoreLabel = Image.renderText(
        dmmonoFont,
        32,
        "JSR Score",
        COLOR_BLACK,
      );
      const scoreColor =
        SCORE_CLASSNAME_TO_COLOR_MAP[getScoreTextColorClass(pkg.score ?? 0)];
      const jsrScoreText = Image.renderText(
        dmmonoFont,
        60,
        `${pkg.score}%`,
        scoreColor,
      );

      const result = new Image(
        Math.max(jsrScoreLabel.width, jsrScoreText.width),
        jsrScoreLabel.height + 10 + jsrScoreText.height,
      );
      result.composite(jsrScoreLabel, 0, 0);
      result.composite(jsrScoreText, 0, jsrScoreLabel.height + 10);
      return result;
    })();

    // Runtime compats
    const runtimeCompats = await (async () => {
      const runtimeCompatsText = Image.renderText(
        dmmonoFont,
        32,
        "Works with",
        COLOR_BLACK,
      );
      const questionMark = Image.renderText(
        dmmonoFont,
        50,
        "?",
        Image.rgbToColor(29, 78, 216),
      );
      let runtimeCompatsImageWidth = 0;
      let runtimeCompatsImageHeight = 0;
      const runtimeKeyImages: Image[] = [];
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
        runtimeKeyImages.push(supportedIcon);
        runtimeCompatsImageWidth += supportedIcon.width;
        runtimeCompatsImageHeight = Math.max(
          runtimeCompatsImageHeight,
          supportedIcon.height,
        );
      }

      const result = new Image(
        Math.max(runtimeCompatsText.width, runtimeCompatsImageWidth),
        runtimeCompatsText.height + 10 + runtimeCompatsImageHeight,
      );

      result.composite(runtimeCompatsText, 0, 0);

      let x = 0;
      for (const runtimeKeyImage of runtimeKeyImages) {
        result.composite(runtimeKeyImage, x, runtimeCompatsText.height + 10);
        x += runtimeKeyImage.width;
      }
      return result;
    })();

    const packageInfomationDefaultY = ((
          (HEIGHT - JSR_LOGO_HEIGHT - PADDING) - // JSR Logo y position
          (descriptionY + descriptionText.height) // Description underline y position
        ) -
          Math.max(
            publishedImage.height,
            jsrScore.height,
            runtimeCompats.height,
          )) / 2 + descriptionY + descriptionText.height;

    const packageInfomationPadding =
      (WIDTH - PADDING * 2 - publishedImage.width - jsrScore.width -
        runtimeCompats.width) / 2;

    ogpImage.composite(publishedImage, PADDING, packageInfomationDefaultY)
      .composite(
        jsrScore,
        PADDING + publishedImage.width + packageInfomationPadding,
        packageInfomationDefaultY,
      )
      .composite(
        runtimeCompats,
        PADDING + publishedImage.width + packageInfomationPadding * 2 +
          jsrScore.width,
        packageInfomationDefaultY,
      );

    // JSR Brand
    const logoWidth = jsrLogo.width * JSR_LOGO_HEIGHT / jsrLogo.height;
    ogpImage.composite(
      jsrLogo.resize(logoWidth, JSR_LOGO_HEIGHT),
      WIDTH - logoWidth - PADDING,
      HEIGHT - JSR_LOGO_HEIGHT - PADDING,
    );

    return new Response(await ogpImage.encode());
  },
};

export const config: RouteConfig = {
  routeOverride: "/@:scope/:package{@:version}?/og",
};
