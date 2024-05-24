import { walkSync } from "jsr:@std/fs@^0.221.0/walk";

if (false) await import(`./routes/${null}`);

const files = walkSync("./routes", {
  includeDirs: false,
  includeSymlinks: false,
});

console.time("route loading");
for (const entry of files) {
  const path = entry.path;
  if (path.endsWith(".tsx")) {
    await import("./" + path);
  }
}
console.timeEnd("route loading");
