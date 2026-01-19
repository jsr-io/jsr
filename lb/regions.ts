// Copyright 2024 the JSR authors. All rights reserved. MIT license.

import type { WorkerEnv } from "./types.ts";

const CF_TO_GCP: Record<string, string> = {
  // North America -> us-central1
  DFW: "us-central1",
  IAD: "us-central1",
  ORD: "us-central1",
  ATL: "us-central1",
  MIA: "us-central1",
  LAX: "us-central1",
  SJC: "us-central1",
  SEA: "us-central1",
  DEN: "us-central1",
  YYZ: "us-central1",
  YVR: "us-central1",

  // Europe -> europe-west1
  AMS: "europe-west1",
  LHR: "europe-west1",
  FRA: "europe-west1",
  CDG: "europe-west1",
  MAD: "europe-west1",
  MAN: "europe-west1",
  ARN: "europe-west1",
  CPH: "europe-west1",
  VIE: "europe-west1",
  WAW: "europe-west1",

  // Asia Pacific - Japan -> asia-northeast1
  NRT: "asia-northeast1",
  KIX: "asia-northeast1",

  // Asia Pacific - India -> asia-south1
  BOM: "asia-south1",
  DEL: "asia-south1",
  MAA: "asia-south1",

  // Asia Pacific - Singapore -> asia-southeast1
  SIN: "asia-southeast1",
  KUL: "asia-southeast1",
  BKK: "asia-southeast1",
  MNL: "asia-southeast1",
  HKG: "asia-southeast1",
  TPE: "asia-southeast1",

  // South America -> southamerica-east1
  GRU: "southamerica-east1",
  SCL: "southamerica-east1",
  EZE: "southamerica-east1",
  BOG: "southamerica-east1",
  LIM: "southamerica-east1",

  // Australia -> australia-southeast1
  SYD: "australia-southeast1",
  MEL: "australia-southeast1",
  PER: "australia-southeast1",
  AKL: "australia-southeast1",
} as const;

export function getFrontendUrl(
  request: Request,
  env: WorkerEnv,
): { region: string; url: string } {
  const colo = (request.cf?.colo as string) || "";
  const region: string = CF_TO_GCP[colo] || "us-central1";

  const frontendRegions = JSON.parse(env.REGISTRY_FRONTEND_URLS) as Record<
    string,
    string
  >;
  const url = frontendRegions[region] || frontendRegions["us-central1"];

  return { region, url };
}
