import type postcss from "npm:postcss@8.4.35";
import type { Config } from "npm:tailwindcss@^3.4.1";
import * as path from "jsr:@std/path@^0.221.0";
import type { TailwindPluginOptions } from "./types.ts";
import type { ResolvedFreshConfig } from "jsr:@fresh/core@^2.0.0-alpha.1";

const CONFIG_EXTENSIONS = ["ts", "js", "mjs"];

async function findTailwindConfigFile(directory: string): Promise<string> {
  let dir = directory;
  while (true) {
    for (let i = 0; i < CONFIG_EXTENSIONS.length; i++) {
      const ext = CONFIG_EXTENSIONS[i];
      const filePath = path.join(dir, `tailwind.config.${ext}`);
      try {
        const stat = await Deno.stat(filePath);
        if (stat.isFile) {
          return filePath;
        }
      } catch (err) {
        if (!(err instanceof Deno.errors.NotFound)) {
          throw err;
        }
      }
    }

    const parent = path.dirname(dir);
    if (parent === dir) {
      throw new Error(
        `Could not find a tailwind config file in the current directory or any parent directory.`,
      );
    }

    dir = parent;
  }
}

export async function initTailwind(
  config: ResolvedFreshConfig,
  options: TailwindPluginOptions,
): Promise<postcss.Processor> {
  const root = path.dirname(config.staticDir);

  const configPath = await findTailwindConfigFile(root);
  const url = path.toFileUrl(configPath).href;
  const tailwindConfig = (await import(url)).default as Config;

  if (!Array.isArray(tailwindConfig.content)) {
    throw new Error(`Expected tailwind "content" option to be an array`);
  }

  // deno-lint-ignore no-explicit-any
  tailwindConfig.content = tailwindConfig.content.map((pattern: any) => {
    if (typeof pattern === "string") {
      const relative = path.relative(Deno.cwd(), path.dirname(configPath));

      if (!relative.startsWith("..")) {
        return path.join(relative, pattern);
      }
    }
    return pattern;
  });

  const [tailwindCss, autoprefixer, cssnano, postcss] = await Promise.all([
    import("npm:tailwindcss@^3.4.1").then((mod) => mod.default),
    import("npm:autoprefixer@10.4.17").then((mod) => mod.default),
    import("npm:cssnano@6.0.3").then((mod) => mod.default),
    import("npm:postcss@8.4.35").then((mod) => mod.default),
  ]);

  // PostCSS types cause deep recursion
  const plugins = [
    // deno-lint-ignore no-explicit-any
    tailwindCss(tailwindConfig) as any,
    // deno-lint-ignore no-explicit-any
    autoprefixer(options.autoprefixer) as any,
  ];

  if (config.mode === "build") {
    plugins.push(cssnano());
  }

  return postcss(plugins);
}
