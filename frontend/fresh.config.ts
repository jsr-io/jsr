// Copyright 2024 the JSR authors. All rights reserved. MIT license.
import { defineConfig, Plugin, PluginRoute } from "$fresh/server.ts";
import { asset } from "$fresh/runtime.ts";
import tailwind from "$fresh/plugins/tailwind.ts";
import { join } from "$std/path";
import { CSS } from "$gfm";

export default defineConfig({
  plugins: [tailwind(), assetifyCssUrl(), gfmCss()],
});

const CSS_URL_REGEX =
  /url\((?:(?<quote>['"])(?<quoted>(?:(?!\k<quote>|\\).|\\.)*)\k<quote>|(?<unquoted>[^'")]*))\)/g;

// This plugin reads the generated style.css file from tailwind plugin and
// replaces the url() (for font paths) with paths that include asset queries for
// caching and cache busting.
function assetifyCssUrl() {
  let outDir: string;
  return {
    name: "assetify-css-url",
    buildStart(config) {
      outDir = config.build.outDir;
    },
    async buildEnd() {
      const stylePath = join(outDir, "static", "styles.css");
      let styleCss = await Deno.readTextFile(stylePath);
      styleCss = styleCss.replaceAll(CSS_URL_REGEX, (...args) => {
        const groups = args.at(-1) as Record<string, string>;
        let path: string;
        if (groups.quoted) {
          path = groups.quoted.replaceAll(/\\./g, (s) => JSON.parse(`"${s}"`));
        } else {
          path = groups.unquoted;
        }
        return `url(${JSON.stringify(asset(path))})`;
      });
      await Deno.writeTextFile(stylePath, styleCss);
    },
  } satisfies Plugin;
}

function gfmCss() {
  const patchedCSS = CSS.replaceAll("font-size:16px;", "");
  const css = /*css*/ `${patchedCSS}
.markdown-body {
	line-height: 1.6;
	overflow: visible;
}

.markdown-body a {
	text-decoration: underline;
}

.markdown-body :where(b, strong) {
	font-weight: 650;
}

.markdown-body ul {
	list-style: disc;
}
.markdown-body ol {
	list-style: numeric;
}

@media screen and (max-width: 768px) {
	.markdown-body pre,
	.markdown-body .highlight pre {
		border-left: 0;
		border-right: 0;
		border-radius: 0;
	}

	.markdown-body pre {
		margin-inline: -1rem;
	}
}

@media screen and (max-width: 1024px) {
	.markdown-body.break pre > code {
		white-space: break-spaces;
		word-break: break-word;
	}
}

.markdown-body table {
	width: fit-content;
}

.markdown-body h2 {
	padding-bottom: 0.375em;
}

.markdown-body h2,
.markdown-body h3 {
	margin-top: 2em;
}

.markdown-body pre {
	border: 1.5px solid #cbd5e1;
}

@media screen and (min-width: 1024px) {
	.markdown-body .highlight pre,
	.markdown-body pre {
		padding: 1.5rem;
	}
}

.markdown-body blockquote {
	padding: 1.5rem;
	background: #f1f5f9; /* cyan-200 */
}

.markdown-body p,
.markdown-body blockquote,
.markdown-body ul,
.markdown-body ol,
.markdown-body dl,
.markdown-body table,
.markdown-body pre,
.markdown-body details,
.markdown-body .highlight {
	margin-bottom: 1.25rem;
}
`;

  let isDev = false;

  return {
    name: "gfm-css",
    configResolved(config) {
      isDev = config.dev;
    },
    async buildStart(config) {
      const outDir = config.build.outDir;
      const stylePath = join(outDir, "static", "gfm.css");
      await Deno.writeTextFile(stylePath, css);
    },
    get routes() {
      if (!isDev) return [];
      return [
        {
          path: "/gfm.css",
          handler: () =>
            new Response(css, { headers: { "content-type": "text/css" } }),
        } satisfies PluginRoute,
      ];
    },
  } satisfies Plugin;
}
