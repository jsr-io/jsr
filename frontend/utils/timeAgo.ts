// Copyright 2024 the JSR authors. All rights reserved. MIT license.
import { difference } from "@std/datetime/difference";
import type { Unit } from "@std/datetime/difference";

const units = [
  "years",
  "months",
  "weeks",
  "days",
  "hours",
  "minutes",
  "seconds",
] as Unit[];

export function timeAgo(date: string): string {
  const duration = difference(new Date(date), new Date(), { units });
  if (duration.seconds === 0) return "0 seconds ago";
  const largestUnit = units.find((unit) => duration[unit]! > 0) || "seconds";
  // @ts-ignore - TS doesn't know about this API yet
  return new Intl.DurationFormat("en", { style: "long" })
    .format({ [largestUnit]: duration[largestUnit] }) + " ago";
}
