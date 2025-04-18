// Copyright 2024 the JSR authors. All rights reserved. MIT license.
export function timeAgo(date: Date | string): string {
  const now = new Date();
  const past = new Date(date);
  const diff = Math.abs(now.getTime() - past.getTime());

  const duration = {
    years: Math.floor(diff / (1000 * 60 * 60 * 24 * 365)),
    months: Math.floor(
      (diff % (1000 * 60 * 60 * 24 * 365)) / (1000 * 60 * 60 * 24 * 30),
    ),
    days: Math.floor(
      (diff % (1000 * 60 * 60 * 24 * 30)) / (1000 * 60 * 60 * 24),
    ),
    hours: Math.floor((diff % (1000 * 60 * 60 * 24)) / (1000 * 60 * 60)),
    minutes: Math.floor((diff % (1000 * 60 * 60)) / (1000 * 60)),
    seconds: Math.floor((diff % (1000 * 60)) / 1000),
  };

  // Force english because JSR is an English-only project
  // @ts-ignore - TS doesn't know about this API yet
  const formatter = new Intl.DurationFormat("en", { style: "long" });

  if (duration.years >= 1) {
    return formatter.format({ years: duration.years }) + " ago";
  } else if (duration.months >= 1) {
    return formatter.format({ months: duration.months }) + " ago";
  } else if (duration.days >= 1) {
    return formatter.format({ days: duration.days }) + " ago";
  } else if (duration.hours >= 1) {
    return formatter.format({ hours: duration.hours }) + " ago";
  } else if (duration.minutes >= 1) {
    return formatter.format({ minutes: duration.minutes }) + " ago";
  } else {
    return formatter.format({ seconds: duration.seconds }) + " ago";
  }
}
