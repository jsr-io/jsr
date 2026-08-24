// Copyright 2024 the JSR authors. All rights reserved. MIT license.

import { assertEquals } from "@std/assert";
import { trackNPMDownload } from "./analytics.ts";
import type { WorkerEnv } from "./types.ts";

function envCapturingDownloads(dataPoints: unknown[]): WorkerEnv {
  return {
    DOWNLOADS: {
      writeDataPoint(dataPoint: unknown) {
        dataPoints.push(dataPoint);
      },
    },
  } as unknown as WorkerEnv;
}

Deno.test("trackNPMDownload tracks npm layout tarball paths", () => {
  const dataPoints: unknown[] = [];
  trackNPMDownload(
    "/@jsr/luca__cases/-/luca__cases-1.0.0.tgz",
    "npm/10.0.0",
    envCapturingDownloads(dataPoints),
  );
  assertEquals(dataPoints, [{
    blobs: ["npm", "luca", "cases", "1.0.0", "npm/10.0.0"],
    indexes: ["@luca/cases"],
  }]);
});

Deno.test("trackNPMDownload tracks legacy revisioned tarball paths", () => {
  const dataPoints: unknown[] = [];
  trackNPMDownload(
    "/~/11/@jsr/luca__cases/1.0.0.tgz",
    null,
    envCapturingDownloads(dataPoints),
  );
  assertEquals(dataPoints, [{
    blobs: ["npm", "luca", "cases", "1.0.0", "n/a"],
    indexes: ["@luca/cases"],
  }]);
});

Deno.test("trackNPMDownload ignores non-tarball paths", () => {
  const dataPoints: unknown[] = [];
  const env = envCapturingDownloads(dataPoints);
  trackNPMDownload("/@jsr/luca__cases", null, env);
  trackNPMDownload("/@jsr/luca__cases/-/other__pkg-1.0.0.tgz", null, env);
  trackNPMDownload("/root.json", null, env);
  assertEquals(dataPoints, []);
});
