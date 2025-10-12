const thousand = 1_000;
const million = 1_000_000;

const abbreviations = new Map<number, string>([
  [thousand, "k"],
  [million, "m"],
]);

/**
 * Format an integer to a short string.
 * The output will at most have 4 characters, including the decimal point and the abbreviation.
 */
export const numberFormat = (num: number): string => {
  num = Math.floor(num);
  if (num >= million) {
    if (num === million) {
      return `1.0${abbreviations.get(million)}`;
    }
    const whole = Math.floor(num / million);
    const remainder = num % million;
    const decimal = Math.floor(remainder / 100000);
    // Show decimal for 1.x m numbers, but not for 10+ m numbers
    return whole < 10 && decimal >= 5
      ? `${whole}.${decimal}${abbreviations.get(million)}`
      : `${whole}${abbreviations.get(million)}`;
  } else if (num >= thousand) {
    if (num === thousand) {
      return `1.0${abbreviations.get(thousand)}`;
    }
    const whole = Math.floor(num / thousand);
    const remainder = num % thousand;
    const decimal = Math.floor(remainder / 100);
    // Show decimal for 1.x k to 9.x k numbers, but not for 10+ k numbers
    return whole < 10 && decimal >= 5
      ? `${whole}.${decimal}${abbreviations.get(thousand)}`
      : `${whole}${abbreviations.get(thousand)}`;
  }
  return num.toString();
};
