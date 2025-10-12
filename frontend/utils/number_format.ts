const scales: [number, string][] = [
  [1_000_000_000, "b"],
  [1_000_000, "m"],
  [1_000, "k"],
];

/**
 * Format an integer to a short string.
 * The output will at most have 4 characters, including the decimal point and the abbreviation.
 * Max 999b.
 */
export const numberFormat = (num: number): string => {
  if (Math.abs(num) < 1000) return Math.floor(num).toString();

  for (const [scale, suffix] of scales) {
    if (Math.abs(num) >= scale) {
      const scaled = num / scale;
      if (scaled < 10) {
        // Truncate to one decimal without rounding, add a small epsilon to avoid floating point precision issues
        const truncated = Math.floor(scaled * 10 + 1e-8) / 10;
        return `${truncated.toFixed(1)}${suffix}`;
      }

      return `${Math.floor(scaled)}${suffix}`;
    }
  }

  return num.toString();
};
