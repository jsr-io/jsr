// Copyright 2024 the JSR authors. All rights reserved. MIT license.
import type { Config, CSSRuleObject } from "tailwindcss/types/config.d.ts";
import colors from "tailwindcss/colors.js";
import plugin from "tailwindcss/plugin.js";
import tailwindPkgJson from "tailwindcss/package.json" with { type: "json" };
import postcss from "postcss";

export default {
  content: [
    "{routes,islands,components}/**/*.{ts,tsx}",
  ],
  plugins: [
    ...(typeof Deno !== "undefined" ? [rewritePreflight()] : []),
  ],
  theme: {
    fontFamily: {
      mono: [
        "DM Mono",
        "Menlo",
        "Monaco",
        '"Lucida Console"',
        "Consolas",
        '"Liberation Mono"',
        '"Courier New"',
        "monospace",
      ],
      sans: [
        "DM Sans",
        "Inter",
        "system-ui",
        "-apple-system",
        "BlinkMacSystemFont",
        '"Segoe UI"',
        "Roboto",
        '"Helvetica Neue"',
        "Arial",
        '"Noto Sans"',
        "sans-serif",
      ],
      serif: [
        "DM Serif",
        "Georgia",
        "Cambria",
        '"Times New Roman"',
        "Times",
        "serif",
      ],
    },
    extend: {
      colors: {
        transparent: "transparent",
        gray: colors.neutral,
        "jsr-yellow": {
          DEFAULT: "#f7df1e",
          "50": "#fefee8",
          "100": "#fdfdc4",
          "200": "#fcf98c",
          "300": "#faee4a",
          "400": "#f7df1e",
          "500": "#e7c50b",
          "600": "#c79a07",
          "700": "#9f7009",
          "800": "#835710",
          "900": "#704713",
          "950": "#412507",
        },
        "jsr-gray": {
          DEFAULT: "#121417",
          0: "#e7e8e8",
          100: "#b8b9b9",
          200: "#a0a1a2",
          300: "#898a8b",
          400: "#717274",
          500: "#595b5d",
          600: "#414345",
          700: "#2a2c2e",
          800: "#121417",
          900: "#0e1012",
        },
        "jsr-cyan": {
          DEFAULT: "#083344",
          "50": "#ebf6ff",
          "100": "#cde9fe",
          "200": "#a6d8fc",
          "300": "#67bef9",
          "400": "#209fee",
          "500": "#0789d5",
          "600": "#0875af",
          "700": "#0e6590",
          "800": "#155775",
          "900": "#164d64",
          "950": "#083344",
        },
        "magenta": {
          50: "#FCE7F3",
          100: "#F8C6E7",
          200: "#F39DCE",
          300: "#F06BA5",
          400: "#E83A7C",
          500: "#DC0953",
          600: "#BF0840",
          700: "#9E072F",
          800: "#7B0620",
          900: "#590510",
          950: "#3B0306",
        },
      },
      spacing: {
        1.75: "0.4375rem",
        4.5: "1.125rem",
        72: "18rem",
        88: "22rem",
      },
      borderWidth: {
        "0": "0",
        "1": "1px",
        "1.5": "1.5px",
      },
      boxShadow: {
        "accent": `8px 14px 0 0 #64748b55`,
        "accent-sm": `5px 6px 0 0 #64748b55`,
        "accent-sm-close": `1px 2px 0 0 #64748b55`,
      },
      gridTemplateColumns: {
        "15": "repeat(15, minmax(0, 1fr))",
      },
      animation: {
        "fade-in": "fade-in 0.7s cubic-bezier(0, 0.63, 0.5, 1) forwards",
        "fade-in-late": "fade-in-late 1s ease-in",
        "scroll-x": "scroll-x 10s linear infinite",
        "scroll-y": "scroll-y 10s linear infinite",
        "rotate-180": "rotate-180 0.3s ease-in-out",
      },
      keyframes: {
        "fade-in": {
          "0%": { opacity: "0", transform: "translateY(1rem)" },
          "100%": { opacity: "1", transform: "translateY(0)" },
        },
        "fade-in-late": {
          "0%": { opacity: "0" },
          "75%": { opacity: "0" },
          "100%": { opacity: "1" },
        },
        "scroll-x": {
          "0%": { transform: "translateX(0)" },
          "100%": { transform: "translateX(-50%)" },
        },
        "scroll-y": {
          "0%": { transform: "translateY(0)" },
          "100%": { transform: "translateY(-50%)" },
        },
        "rotate-180": {
          "0%": { transform: "rotate(0deg)" },
          "100%": { transform: "rotate(180deg)" },
        },
      },
    },
  },
} satisfies Config;

function rewritePreflight() {
  return plugin(({ addBase }) => {
    const preflight = postcss.parse(
      Deno.readTextFileSync("./node_modules/tailwindcss/lib/css/preflight.css"),
    );

    // Tailwindcss applies `height: auto` for img and video tags in preflight css,
    // which disrupts the common practice of resizing medias in markdown through the height attributes,
    // because the height property in CSS has a higher priority than the DOM attribute.
    //
    // This should be able to be safely removed,
    // see: https://github.com/tailwindlabs/tailwindcss/pull/7742#issuecomment-1061332148
    preflight.walkRules(/^img,\s*video$/, (rule) => {
      rule.nodes = rule.nodes.filter((node) =>
        !(node.type === "decl" && node.prop === "height" &&
          node.value === "auto")
      );
      preflight.insertAfter(
        rule,
        "img:where(:not(.markdown img)), video:where(:not(.markdown video)) { height: auto; }",
      );
    });

    addBase([
      postcss.comment({
        text:
          `! tailwindcss v${tailwindPkgJson.version} | MIT License | https://tailwindcss.com`,
      }) as unknown as CSSRuleObject,
      ...preflight.nodes as unknown as CSSRuleObject[],
    ]);
  }, {
    corePlugins: {
      preflight: false,
    },
  });
}
