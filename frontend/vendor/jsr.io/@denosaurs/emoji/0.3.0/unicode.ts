/** Used to compose unicode character classes. */
export const rsAstralRange = "\\ud800-\\udfff",
  rsComboMarksRange = "\\u0300-\\u036f",
  reComboHalfMarksRange = "\\ufe20-\\ufe2f",
  rsComboSymbolsRange = "\\u20d0-\\u20ff",
  rsComboRange = rsComboMarksRange + reComboHalfMarksRange +
    rsComboSymbolsRange,
  rsDingbatRange = "\\u2700-\\u27bf",
  rsLowerRange = "a-z\\xdf-\\xf6\\xf8-\\xff",
  rsMathOpRange = "\\xac\\xb1\\xd7\\xf7",
  rsNonCharRange = "\\x00-\\x2f\\x3a-\\x40\\x5b-\\x60\\x7b-\\xbf",
  rsPunctuationRange = "\\u2000-\\u206f",
  rsSpaceRange =
    " \\t\\x0b\\f\\xa0\\ufeff\\n\\r\\u2028\\u2029\\u1680\\u180e\\u2000\\u2001\\u2002\\u2003\\u2004\\u2005\\u2006\\u2007\\u2008\\u2009\\u200a\\u202f\\u205f\\u3000",
  rsUpperRange = "A-Z\\xc0-\\xd6\\xd8-\\xde",
  rsVarRange = "\\ufe0e\\ufe0f",
  rsBreakRange = rsMathOpRange + rsNonCharRange + rsPunctuationRange +
    rsSpaceRange;

export const rsApos = "['\u2019]",
  rsAstral = "[" + rsAstralRange + "]",
  rsBreak = "[" + rsBreakRange + "]",
  rsCombo = "[" + rsComboRange + "]",
  rsDigits = "\\d+",
  rsDingbat = "[" + rsDingbatRange + "]",
  rsLower = "[" + rsLowerRange + "]",
  rsMisc = "[^" +
    rsAstralRange +
    rsBreakRange +
    rsDigits +
    rsDingbatRange +
    rsLowerRange +
    rsUpperRange +
    "]",
  rsFitz = "\\ud83c[\\udffb-\\udfff]",
  rsModifier = "(?:" + rsCombo + "|" + rsFitz + ")",
  rsNonAstral = "[^" + rsAstralRange + "]",
  rsRegional = "(?:\\ud83c[\\udde6-\\uddff]){2}",
  rsSurrPair = "[\\ud800-\\udbff][\\udc00-\\udfff]",
  rsUpper = "[" + rsUpperRange + "]",
  rsZWJ = "\\u200d";

/** Used to compose unicode regexes. */
export const rsMiscLower = "(?:" + rsLower + "|" + rsMisc + ")",
  rsMiscUpper = "(?:" + rsUpper + "|" + rsMisc + ")",
  rsOptContrLower = "(?:" + rsApos + "(?:d|ll|m|re|s|t|ve))?",
  rsOptContrUpper = "(?:" + rsApos + "(?:D|LL|M|RE|S|T|VE))?",
  reOptMod = rsModifier + "?",
  rsOptconst = "[" + rsVarRange + "]?",
  rsOptJoin = "(?:" +
    rsZWJ +
    "(?:" +
    [rsNonAstral, rsRegional, rsSurrPair].join("|") +
    ")" +
    rsOptconst +
    reOptMod +
    ")*",
  rsOrdLower = "\\d*(?:1st|2nd|3rd|(?![123])\\dth)(?=\\b|[A-Z_])",
  rsOrdUpper = "\\d*(?:1ST|2ND|3RD|(?![123])\\dTH)(?=\\b|[a-z_])",
  rsSeq = rsOptconst + reOptMod + rsOptJoin,
  rsEmoji = "(?:" + [rsDingbat, rsRegional, rsSurrPair].join("|") + ")" + rsSeq,
  rsSymbol = "(?:" +
    [
      rsNonAstral + rsCombo + "?",
      rsCombo,
      rsRegional,
      rsSurrPair,
      rsAstral,
    ].join("|") +
    ")";

/** Used to match apostrophes. */
export const reApos = new RegExp(rsApos, "g");

/**
 * Used to match [combining diacritical marks](https://en.wikipedia.org/wiki/Combining_Diacritical_Marks) and
 * [combining diacritical marks for symbols](https://en.wikipedia.org/wiki/Combining_Diacritical_Marks_for_Symbols).
 */
export const reComboMark = new RegExp(rsCombo, "g");

/** Used to match [string symbols](https://mathiasbynens.be/notes/javascript-unicode). */
export const reUnicode = new RegExp(
  rsFitz + "(?=" + rsFitz + ")|" + rsSymbol + rsSeq,
  "g",
);

export const reEmoji = new RegExp(rsEmoji, "g");

/** Used to match complex or compound words. */
export const reUnicodeWord = new RegExp(
  [
    rsUpper +
    "?" +
    rsLower +
    "+" +
    rsOptContrLower +
    "(?=" +
    [rsBreak, rsUpper, "$"].join("|") +
    ")",
    rsMiscUpper +
    "+" +
    rsOptContrUpper +
    "(?=" +
    [rsBreak, rsUpper + rsMiscLower, "$"].join("|") +
    ")",
    rsUpper + "?" + rsMiscLower + "+" + rsOptContrLower,
    rsUpper + "+" + rsOptContrUpper,
    rsOrdUpper,
    rsOrdLower,
    rsDigits,
    rsEmoji,
  ].join("|"),
  "g",
);

/** Used to detect strings with [zero-width joiners or code points from the astral planes](http://eev.ee/blog/2015/09/12/dark-corners-of-unicode/). */
export const reHasUnicode = new RegExp(
  "[" + rsZWJ + rsAstralRange + rsComboRange + rsVarRange + "]",
);
