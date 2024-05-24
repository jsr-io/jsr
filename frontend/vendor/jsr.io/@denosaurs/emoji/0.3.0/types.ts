/**
 * emoji-test: https://unicode.org/Public/emoji/14.0/emoji-test.txt
 * cldr-annotations: https://raw.githubusercontent.com/unicode-org/cldr-json/main/cldr-json/cldr-annotations-full/annotations/en/annotations.json
 * Unicode/Emoji Versioning: https://unicode.org/reports/tr51/proposed.html#Versioning
 *
 * @property emoji - The actual emoji char (emoji-test)
 * @property description - A description of the emoji (emoji-test)
 * @property group - For categorizartion (emoji-test)
 * @property subgroup - For categorizartion (emoji-test)
 * @property emojiVersion - Emoji version emoji was introduced (emoji-test)
 * @property unicodeVersion - Unicode version emoji was introduced (Unicode/Emoji Versioning)
 * @property tags - Related keywords (cldr-annotations)
 * @property aliases - Unique identifier (cldr-annotations)
 * @property skinTones - Whether emoji has skin-tone variants (emoji-test)
 */
export interface Emoji {
  emoji: string;
  description: string;
  group: string;
  subgroup: string;
  emojiVersion: number;
  unicodeVersion: number;
  tags: string[];
  aliases: string[];
  skinTones?: boolean;
}
