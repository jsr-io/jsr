import { emojify } from "jsr:@denosaurs/emoji@0.3";
import * as Marked from "npm:marked@^12";
import GitHubSlugger from "npm:github-slugger@^2.0";
import markedAlert from "npm:marked-alert@^2.0";
import markedFootnote from "npm:marked-footnote@^1.2";
import { gfmHeadingId } from "npm:marked-gfm-heading-id@^3.1";
import Prism from "npm:prismjs@^1.29";
import sanitizeHtml from "npm:sanitize-html@^2.11";
import he from "npm:he@^1.2";
import katex from "npm:katex@^0.16";

import { CSS, KATEX_CLASSES, KATEX_CSS } from "./style.ts";
export { CSS, KATEX_CSS, Marked };

Marked.marked.use(markedAlert());
Marked.marked.use(gfmHeadingId());
Marked.marked.use(markedFootnote());
Marked.marked.use({
  walkTokens: (token) => {
    // putting a list inside a summary requires a double line break
    // but we shouldn't keep that double line break in the output
    // this doesn't happen in remark/rehype
    if (token.type === "html" && token.text.endsWith("</summary>\n\n")) {
      token.text = token.text.replace("</summary>\n\n", "</summary>\n");
    }
  },
});

export class Renderer extends Marked.Renderer {
  allowMath: boolean;
  baseUrl: string | undefined;
  #slugger: GitHubSlugger;

  constructor(options: Marked.MarkedOptions & RenderOptions = {}) {
    super(options);
    this.baseUrl = options.baseUrl;
    this.allowMath = options.allowMath ?? false;
    this.#slugger = new GitHubSlugger();
  }

  heading(
    text: string,
    level: 1 | 2 | 3 | 4 | 5 | 6,
    raw: string,
  ): string {
    const slug = this.#slugger.slug(raw);
    return `<h${level} id="${slug}"><a class="anchor" aria-hidden="true" tabindex="-1" href="#${slug}"><svg class="octicon octicon-link" viewBox="0 0 16 16" width="16" height="16" aria-hidden="true"><path fill-rule="evenodd" d="M7.775 3.275a.75.75 0 001.06 1.06l1.25-1.25a2 2 0 112.83 2.83l-2.5 2.5a2 2 0 01-2.83 0 .75.75 0 00-1.06 1.06 3.5 3.5 0 004.95 0l2.5-2.5a3.5 3.5 0 00-4.95-4.95l-1.25 1.25zm-4.69 9.64a2 2 0 010-2.83l2.5-2.5a2 2 0 012.83 0 .75.75 0 001.06-1.06 3.5 3.5 0 00-4.95 0l-2.5 2.5a3.5 3.5 0 004.95 4.95l1.25-1.25a.75.75 0 00-1.06-1.06l-1.25 1.25a2 2 0 01-2.83 0z"></path></svg></a>${text}</h${level}>\n`;
  }

  image(src: string, title: string | null, alt: string): string {
    return `<img src="${src}" alt="${alt}" title="${title ?? ""}" />`;
  }

  code(code: string, language?: string): string {
    // a language of `ts, ignore` should really be `ts`
    // and it should be lowercase to ensure it has parity with regular github markdown
    language = language?.split(",")?.[0].toLocaleLowerCase();

    // transform math code blocks into HTML+MathML
    // https://github.blog/changelog/2022-06-28-fenced-block-syntax-for-mathematical-expressions/
    if (language === "math" && this.allowMath) {
      return katex.renderToString(code, { displayMode: true });
    }
    const grammar =
      language && Object.hasOwnProperty.call(Prism.languages, language)
        ? Prism.languages[language]
        : undefined;
    if (grammar === undefined) {
      return `<pre><code class="notranslate">${he.encode(code)}</code></pre>`;
    }
    const html = Prism.highlight(code, grammar, language!);
    return `<div class="highlight highlight-source-${language} notranslate"><pre>${html}</pre></div>`;
  }

  link(href: string, title: string | null, text: string): string {
    const titleAttr = title ? ` title="${title}"` : "";
    if (href.startsWith("#")) {
      return `<a href="${href}"${titleAttr}>${text}</a>`;
    }
    if (this.baseUrl) {
      try {
        href = new URL(href, this.baseUrl).href;
      } catch (_) {
        //
      }
    }
    return `<a href="${href}"${titleAttr} rel="noopener noreferrer">${text}</a>`;
  }
}

const BLOCK_MATH_REGEXP = /\$\$\s(.+?)\s\$\$/g;
const INLINE_MATH_REGEXP = /\s\$((?=\S).*?(?=\S))\$/g;

/** Convert inline and block math to katex */
function mathify(markdown: string) {
  // Deal with block math
  markdown = markdown.replace(BLOCK_MATH_REGEXP, (match, p1) => {
    try {
      return katex.renderToString(p1.trim(), { displayMode: true });
    } catch (e) {
      console.warn(e);
      // Don't replace the math if there's an error
      return match;
    }
  });

  // Deal with inline math
  markdown = markdown.replace(INLINE_MATH_REGEXP, (match, p1) => {
    try {
      return " " + katex.renderToString(p1, { displayMode: false });
    } catch (e) {
      console.warn(e);
      // Don't replace the math if there's an error
      return match;
    }
  });

  return markdown;
}

function getOpts(opts: RenderOptions) {
  return {
    baseUrl: opts.baseUrl,
    breaks: opts.breaks ?? false,
    gfm: true,
    mangle: false,
    renderer: opts.renderer ? opts.renderer : new Renderer(opts),
    async: false,
  };
}

export interface RenderOptions {
  baseUrl?: string;
  mediaBaseUrl?: string;
  inline?: boolean;
  allowIframes?: boolean;
  allowMath?: boolean;
  disableHtmlSanitization?: boolean;
  renderer?: Renderer;
  allowedClasses?: { [index: string]: boolean | Array<string | RegExp> };
  allowedTags?: string[];
  allowedAttributes?: Record<string, sanitizeHtml.AllowedAttribute[]>;
  breaks?: boolean;
}

export function render(markdown: string, opts: RenderOptions = {}): string {
  opts.mediaBaseUrl ??= opts.baseUrl;
  markdown = emojify(markdown);
  if (opts.allowMath) {
    markdown = mathify(markdown);
  }

  const marked_opts = getOpts(opts);

  const html =
    (opts.inline
      ? Marked.marked.parseInline(markdown, marked_opts)
      : Marked.marked.parse(markdown, marked_opts)) as string;

  if (opts.disableHtmlSanitization) {
    return html;
  }

  let defaultAllowedTags = sanitizeHtml.defaults.allowedTags.concat([
    "img",
    "video",
    "svg",
    "path",
    "circle",
    "figure",
    "figcaption",
    "del",
    "details",
    "summary",
    "input",
  ]);
  if (opts.allowIframes) {
    defaultAllowedTags.push("iframe");
  }
  if (opts.allowMath) {
    defaultAllowedTags = defaultAllowedTags.concat([
      "math",
      "maction",
      "annotation",
      "annotation-xml",
      "menclose",
      "merror",
      "mfenced",
      "mfrac",
      "mi",
      "mmultiscripts",
      "mn",
      "mo",
      "mover",
      "mpadded",
      "mphantom",
      "mprescripts",
      "mroot",
      "mrow",
      "ms",
      "semantics",
      "mspace",
      "msqrt",
      "mstyle",
      "msub",
      "msup",
      "msubsup",
      "mtable",
      "mtd",
      "mtext",
      "mtr",
    ]);
  }

  function transformMedia(tagName: string, attribs: sanitizeHtml.Attributes) {
    if (opts.mediaBaseUrl && attribs.src) {
      try {
        attribs.src = new URL(attribs.src, opts.mediaBaseUrl).href;
      } catch {
        delete attribs.src;
      }
    }
    return { tagName, attribs };
  }

  const defaultAllowedClasses = {
    div: [
      "highlight",
      "highlight-source-*",
      "notranslate",
      "markdown-alert",
      "markdown-alert-*",
    ],
    span: [
      "token",
      "keyword",
      "operator",
      "number",
      "boolean",
      "function",
      "string",
      "comment",
      "class-name",
      "regex",
      "regex-delimiter",
      "tag",
      "attr-name",
      "punctuation",
      "script-punctuation",
      "script",
      "plain-text",
      "property",
      "prefix",
      "line",
      "deleted",
      "inserted",
      ...(opts.allowMath ? KATEX_CLASSES : []),
    ],
    a: ["anchor"],
    p: ["markdown-alert-title"],
    svg: ["octicon", "octicon-alert", "octicon-link"],
    h2: ["sr-only"],
    section: ["footnotes"],
  };

  const defaultAllowedAttributes = {
    ...sanitizeHtml.defaults.allowedAttributes,
    img: ["src", "alt", "height", "width", "align", "title"],
    video: [
      "src",
      "alt",
      "height",
      "width",
      "autoplay",
      "muted",
      "loop",
      "playsinline",
      "poster",
      "controls",
      "title",
    ],
    a: [
      "id",
      "aria-hidden",
      "href",
      "tabindex",
      "rel",
      "target",
      "title",
      "data-footnote-ref",
      "data-footnote-backref",
      "aria-label",
      "aria-describedby",
    ],
    svg: ["viewBox", "width", "height", "aria-hidden", "background"],
    path: ["fill-rule", "d"],
    circle: ["cx", "cy", "r", "stroke", "stroke-width", "fill", "alpha"],
    span: opts.allowMath ? ["aria-hidden", "style"] : [],
    h1: ["id"],
    h2: ["id"],
    h3: ["id"],
    h4: ["id"],
    h5: ["id"],
    h6: ["id"],
    li: ["id"],
    td: ["colspan", "rowspan", "align", "width"],
    iframe: ["src", "width", "height"], // Only used when iframe tags are allowed in the first place.
    math: ["xmlns"], // Only enabled when math is enabled
    annotation: ["encoding"], // Only enabled when math is enabled
    details: ["open"],
    section: ["data-footnotes"],
    input: ["checked", "disabled", {
      name: "type",
      values: ["checkbox"],
    }],
  };

  return sanitizeHtml(html, {
    transformTags: {
      img: transformMedia,
      video: transformMedia,
    },
    allowedTags: [...defaultAllowedTags, ...opts.allowedTags ?? []],
    allowedAttributes: mergeAttributes(
      defaultAllowedAttributes,
      opts.allowedAttributes ?? {},
    ),
    allowedClasses: { ...defaultAllowedClasses, ...opts.allowedClasses },
    allowProtocolRelative: false,
    parser: {
      lowerCaseAttributeNames: false,
    },
  });
}

function mergeAttributes(
  defaults: Record<string, sanitizeHtml.AllowedAttribute[]>,
  customs: Record<string, sanitizeHtml.AllowedAttribute[]>,
) {
  const merged = { ...defaults };
  for (const tag in customs) {
    merged[tag] = [...(merged[tag] || []), ...customs[tag]];
  }
  return merged;
}

function stripTokens(
  tokens: Marked.Token[],
  sections: MarkdownSections[],
  header: boolean,
) {
  let index = sections.length - 1;

  for (const token of tokens) {
    if (token.type === "heading") {
      sections[index].header = sections[index].header.trim().replace(
        /\n{3,}/g,
        "\n",
      );
      sections[index].content = sections[index].content.trim().replace(
        /\n{3,}/g,
        "\n",
      );

      sections.push({ header: "", depth: token.depth, content: "" });
      index += 1;
    }

    if ("tokens" in token && token.tokens) {
      stripTokens(token.tokens, sections, token.type === "heading");
    }

    switch (token.type) {
      case "space":
        sections[index][header ? "header" : "content"] += token.raw;
        break;
      case "code":
        if (token.lang != "math") {
          sections[index][header ? "header" : "content"] += token.text;
        }
        break;
      case "heading":
        break;
      case "table":
        for (const cell of token.header) {
          stripTokens(cell.tokens, sections, header);
          sections[index][header ? "header" : "content"] += " ";
        }
        sections[index][header ? "header" : "content"] += "\n";
        for (const row of token.rows) {
          for (const cell of row) {
            stripTokens(cell.tokens, sections, header);
            sections[index][header ? "header" : "content"] += " ";
          }
          sections[index][header ? "header" : "content"] += "\n";
        }
        break;
      case "hr":
        break;
      case "blockquote":
        break;
      case "list":
        stripTokens(token.items, sections, header);
        break;
      case "list_item":
        sections[index][header ? "header" : "content"] += "\n";
        break;
      case "paragraph":
        break;
      case "html": {
        // TODO: extract alt from img
        sections[index][header ? "header" : "content"] +=
          sanitizeHtml(token.text, {
            allowedTags: [],
            allowedAttributes: {},
          }).trim() + "\n\n";
        break;
      }
      case "text":
        if (!("tokens" in token) || !token.tokens) {
          sections[index][header ? "header" : "content"] += token.raw;
        }
        break;
      case "def":
        break;
      case "escape":
        break;
      case "link":
        break;
      case "image":
        if (token.title) {
          sections[index][header ? "header" : "content"] += token.title;
        } else {
          sections[index][header ? "header" : "content"] += token.text;
        }
        break;
      case "strong":
        break;
      case "em":
        break;
      case "codespan":
        sections[index][header ? "header" : "content"] += token.text;
        break;
      case "br":
        break;
      case "del":
        break;
    }
  }
}

class StripTokenizer extends Marked.Tokenizer {
  codespan(src: string): Marked.Tokens.Codespan | undefined {
    // copied & modified from Marked to remove escaping
    const cap = this.rules.inline.code.exec(src);
    if (cap) {
      let text = cap[2].replace(/\n/g, " ");
      const hasNonSpaceChars = /[^ ]/.test(text);
      const hasSpaceCharsOnBothEnds = /^ /.test(text) && / $/.test(text);
      if (hasNonSpaceChars && hasSpaceCharsOnBothEnds) {
        text = text.substring(1, text.length - 1);
      }
      return {
        type: "codespan",
        raw: cap[0],
        text,
      };
    }
  }
}

export interface MarkdownSections {
  /** The header of the section */
  header: string;
  /** The depth-level of the header. 0 if it is root level */
  depth: number;
  content: string;
}

/**
 * Strip all markdown syntax to get a plaintext output, divided up in sections
 * based on headers
 */
export function stripSplitBySections(
  markdown: string,
  opts: RenderOptions = {},
): MarkdownSections[] {
  markdown = emojify(markdown).replace(BLOCK_MATH_REGEXP, "").replace(
    INLINE_MATH_REGEXP,
    "",
  );
  const tokens = Marked.marked.lexer(markdown, {
    ...getOpts(opts),
    tokenizer: new StripTokenizer(),
  });

  const sections: MarkdownSections[] = [{
    header: "",
    depth: 0,
    content: "",
  }];
  stripTokens(tokens, sections, false);

  return sections;
}

/**
 * Strip all markdown syntax to get a plaintext output
 */
export function strip(markdown: string, opts: RenderOptions = {}): string {
  return stripSplitBySections(markdown, opts).map((section) =>
    section.header + "\n\n" + section.content
  ).join("\n\n").trim().replace(/\n{3,}/g, "\n") + "\n";
}
