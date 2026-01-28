// Copyright 2024 the JSR authors. All rights reserved. MIT license.

import type { WorkerEnv } from "./types.ts";

export function trackJSRDownload(pathname: string, env: WorkerEnv): void {
  const match = pathname.match(/^\/@([^/]+)\/([^/]+)\/([^/]+)_meta\.json$/);
  if (match) {
    const [, scope, packageName, version] = match;
    console.log("tracked download:", [
      "jsr",
      scope,
      packageName,
      version,
    ]);
    env.DOWNLOADS?.writeDataPoint({
      blobs: [
        "jsr",
        scope,
        packageName,
        version,
      ],
      indexes: [`@${scope}/${packageName}`],
    });
  }
}

export function trackNPMDownload(pathname: string, env: WorkerEnv): void {
  const match = pathname.match(
    /^\/~\/\d+\/@jsr\/([^_]+)__([^/]+)\/([^/]+)\.tgz$/,
  );
  if (match) {
    const [, scope, packageName, version] = match;
    env.DOWNLOADS?.writeDataPoint({
      blobs: [
        "npm",
        scope,
        packageName,
        version,
      ],
      indexes: [`@${scope}/${packageName}`],
    });
  }
}
