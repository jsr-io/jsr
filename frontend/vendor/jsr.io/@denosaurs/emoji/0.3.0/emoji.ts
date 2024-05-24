import type { Emoji } from "./types.ts";
import emojis from "./all.json" with { type: "json" };
import { reUnicode } from "./unicode.ts";

export { Emoji };

// Regex to parse emoji in a string - e.g. :coffee:
const reEmojiName = /:([a-zA-Z0-9_\-\+]+):/g;

// Non spacing mark, some emoticons have them. It's the 'Variant Form',
// which provides more information so that emoticons can be rendered as
// more colorful graphics. FE0E is a unicode text version, where as FE0F
// should be rendered as a graphical version. The code gracefully degrades.
const NON_SPACING_MARK = String.fromCharCode(65039); // 65039 - '️' - 0xFE0F;
const reNonSpacing = new RegExp(NON_SPACING_MARK, "g");

/** Remove NON_SPACING_MARK from string. See above. */
function stripNSB(code: string): string {
  return code.replace(reNonSpacing, "");
}

/** Removes colons on either side of the string. */
function stripColons(str: string): string {
  const colonIndex = str.indexOf(":");
  if (colonIndex > -1) {
    if (colonIndex === str.length - 1) {
      str = str.substring(0, colonIndex);
    } else {
      str = str.substring(colonIndex + 1);
    }
    return stripColons(str);
  }

  return str;
}

/** Adds colons to either side of the string. */
function wrapColons(str: string): string {
  return str.length > 0 ? ":" + str + ":" : str;
}

type EmojiMap = { [alias: string]: Emoji };

const byAlias: EmojiMap = Object.fromEntries(
  emojis.map((emoji) => emoji.aliases.map((alias) => [alias, emoji])).flat(),
);

const byCode: EmojiMap = Object.fromEntries(
  emojis.map((emoji) => {
    return [stripNSB(emoji.emoji), emoji];
  }),
);

/** Get all emojis. */
export function all(): Emoji[] {
  return emojis;
}

/** Get all emojis as alias map. */
export function aliasMap(): EmojiMap {
  return byAlias;
}

/** Get all emojis as code map. */
export function codeMap(): EmojiMap {
  return byCode;
}

/** Get emoji from alias. - e.g. "unicorn" -> 🦄 */
export function get(alias: string): string {
  return byAlias[stripColons(alias)]?.emoji;
}

/** Get alias from emoji. - e.g. 👕 -> "shirt" */
export function alias(emoji: string): string {
  return byCode[stripNSB(emoji)]?.aliases[0];
}

/** Get alias from emoji. - e.g. 👕 -> ["shirt", "tshirt"] */
export function aliases(emoji: string): string[] {
  return byCode[stripNSB(emoji)]?.aliases;
}

/** Get emoji info from alias or emoji */
export function infoByAlias(raw: string): Emoji | undefined {
  return byAlias[stripColons(raw)];
}

/** Get emoji info from alias or emoji */
export function infoByCode(raw: string): Emoji | undefined {
  return byCode[stripNSB(raw)];
}

/** Get random emoji. - e.g. {emoji: "👕", alias: "shirt"} */
export function random(): { emoji: string; alias: string } {
  const random = emojis[Math.floor(Math.random() * emojis.length)];
  const emoji = random.emoji;
  const alias = random.aliases[0];
  return { emoji, alias };
}

/** Strip all emojis in a string. */
export function strip(str: string): string {
  return replace(str, "", true);
}

/** Replace all emojis in a string. */
export function replace(
  str: string,
  replacement: string | ((emoji: Emoji) => string) = "",
  trim = false,
): string {
  if (!str) return "";
  const replace = typeof replacement === "function" ? replacement : () => {
    return replacement;
  };
  const match = str.match(reUnicode) ?? [];
  const result = match
    .map((s, i) => {
      const emoji = byCode[stripNSB(s)];
      if (emoji && trim && match[i + 1] === " ") {
        match[i + 1] = "";
      }
      return emoji ? replace(emoji) : s;
    })
    .join("");
  return trim ? result.trim() : result;
}

/** Replace all emoji names in a string with actual emojis. */
export function emojify(str: string): string {
  if (!str) return "";
  return str
    .split(reEmojiName)
    .map((s, i) => {
      if (i % 2 === 0) return s;
      let emoji = get(s);
      if (!emoji) emoji = wrapColons(s);
      return emoji;
    })
    .join("");
}

/** Replace all emoji in a string with actual emoji names. */
export function unemojify(str: string): string {
  return replace(str, (emoji) => wrapColons(emoji.aliases[0]));
}

/** Tagged template version of emojify */
export function emoji(
  template: TemplateStringsArray,
  ...args: string[]
): string {
  const chunks = [];
  for (let i = 0; i < template.length; i++) {
    chunks.push(emojify(template[i]));
    if (args[i]) chunks.push(emojify(args[i]));
  }
  return chunks.join("");
}
