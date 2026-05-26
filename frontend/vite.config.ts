import { defineConfig, type Plugin } from "vite";
import { fresh } from "@fresh/plugin-vite";
import tailwindcss from "@tailwindcss/vite";
import { CSS } from "@deno/gfm";

const MARKER =
  "/*! During the build process, the @deno/gfm CSS file is injected here. */";

function imagescriptUrl(): Plugin {
  return {
    name: "imagescript-url",
    enforce: "pre",
    transform(code, id) {
      const m = id.match(/jsr\.io\/(@matmen\/imagescript\/[^?]+)/);
      if (!m || !code.includes("import.meta.url")) return null;
      const realUrl = `https://jsr.io/${m[1]}`;
      return {
        code: code.replaceAll("import.meta.url", JSON.stringify(realUrl)),
        map: null,
      };
    },
  };
}

function gfmCss(): Plugin {
  return {
    name: "gfm-css",
    enforce: "pre",
    transform(code, id) {
      if (!/\/gfm\.css(?:$|\?)/.test(id)) return null;
      if (!code.includes(MARKER)) return null; // cheap guard
      const injected = CSS.replaceAll("font-size:16px;", "");
      return { code: code.replace(MARKER, injected), map: null };
    },
  };
}

export default defineConfig({
  server: {
    port: 8000,
  },
  plugins: [
    fresh(),
    gfmCss(),
    imagescriptUrl(),
    tailwindcss(),
  ],
});