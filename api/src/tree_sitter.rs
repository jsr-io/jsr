// Copyright 2024 the JSR authors. All rights reserved. MIT license.
use std::collections::HashMap;
use std::io::Write;
use std::ops::Range;
use std::sync::OnceLock;

use tree_sitter_highlight::Highlight;
use tree_sitter_highlight::HighlightConfiguration;
use tree_sitter_highlight::HighlightEvent;

/// A span of the highlighted source that should be rendered as a link, used by
/// the file view to make import/export specifiers clickable (jsr-io/jsr#17).
#[derive(Debug, Clone)]
pub struct SourceLink {
  /// Byte range in the source being highlighted, including the quotes around
  /// the specifier.
  pub range: Range<usize>,
  pub href: String,
}

pub struct ComrakAdapter {
  pub show_line_numbers: bool,
  /// Ranges to wrap in anchors, sorted by start and non-overlapping. Markdown
  /// code fences pass none: their offsets are fence-relative, and only the
  /// file view knows what a specifier resolves to.
  pub links: Vec<SourceLink>,
}

impl ComrakAdapter {
  pub fn new(show_line_numbers: bool) -> Self {
    Self {
      show_line_numbers,
      links: Vec::new(),
    }
  }
}

impl comrak::adapters::SyntaxHighlighterAdapter for ComrakAdapter {
  fn write_highlighted(
    &self,
    output: &mut dyn Write,
    lang: Option<&str>,
    code: &str,
  ) -> std::io::Result<()> {
    let lang = lang.unwrap_or_default();
    let config = tree_sitter_language_cb(lang);
    let source = code.as_bytes();
    if let Some(config) = config {
      let mut highlighter = tree_sitter_highlight::Highlighter::new();
      // unsure why exactly, but without the closure it doesnt compile
      // seems to be related to the static aspect of tree_sitter_language_cb
      #[allow(clippy::redundant_closure)]
      let res = highlighter
        .highlight(config, source, None, |e| tree_sitter_language_cb(e));

      match res {
        Ok(events) => {
          match render_lines(events, source, &self.links) {
            Ok(rendered) => {
              let mut line_numbers = String::new();
              let mut lines = String::new();

              for (i, line) in rendered.iter().enumerate() {
                let n = i + 1;

                if self.show_line_numbers {
                  line_numbers.push_str(&format!(
                    r##"<a href="#L{n}" class="no_color">{n}</a>"##,
                  ));

                  lines.push_str(&format!(r#"<span id="L{n}">"#));
                }

                lines.push_str(line);

                if self.show_line_numbers {
                  lines.push_str("</span>");
                }
              }

              let html = if self.show_line_numbers {
                format!(
                  r##"<div class="lineNumbers">{line_numbers}</div><div class="grow overflow-x-auto"><div class="w-max lineNumbersHighlight">{lines}</div></div>"##
                )
              } else {
                lines
              };

              return output.write_all(html.as_bytes());
            }
            Err(err) => {
              eprintln!("Error rendering code: {}", err);
            }
          };
        }
        Err(err) => {
          eprintln!("Error highlighting code: {}", err);
        }
      }
    }

    comrak::html::escape(output, source)
  }

  fn write_pre_tag(
    &self,
    output: &mut dyn Write,
    attributes: HashMap<String, String>,
  ) -> std::io::Result<()> {
    comrak::html::write_opening_tag(output, "pre", attributes)
  }

  fn write_code_tag(
    &self,
    output: &mut dyn Write,
    mut attributes: HashMap<String, String>,
  ) -> std::io::Result<()> {
    if self.show_line_numbers {
      attributes
        .entry("class".into())
        .or_default()
        .push_str(" flex! gap-2");
    }
    comrak::html::write_opening_tag(output, "code", attributes)
  }
}

macro_rules! highlighter {
    [$($name:literal -> $class:literal,)*] => {
      /// The capture names to configure on the highlighter. If this is not
      /// configured correctly, the highlighter will not work.
      pub const CAPTURE_NAMES: &[&str] = &[$($name),*];
      const CLASSES_ATTRIBUTES: &[&str] = &[$(concat!("class=\"", $class, "\"")),*];
      pub const CLASSES: &[&str] = &[$($class),*];
    };
}

highlighter! [
  "attribute" -> "pl-c1",
  "comment" -> "pl-c",
  "constant.builtin" -> "pl-c1",
  "constant" -> "pl-c1",
  "constructor" -> "pl-v",
  "embedded" -> "pl-s1",
  "function" -> "pl-en",
  "keyword" -> "pl-k",
  "number" -> "pl-c1",
  "operator" -> "pl-c1",
  "property" -> "pl-c1",
  "string" -> "pl-s",
  "tag" -> "pl-ent",
  "type" -> "pl-smi",
  "variable.builtin" -> "pl-smi",
];

pub(crate) fn classes(highlight: Highlight) -> &'static [u8] {
  CLASSES_ATTRIBUTES[highlight.0].as_bytes()
}

const fn html_escape(c: u8) -> Option<&'static str> {
  match c {
    b'>' => Some("&gt;"),
    b'<' => Some("&lt;"),
    b'&' => Some("&amp;"),
    b'\'' => Some("&#39;"),
    b'"' => Some("&quot;"),
    _ => None,
  }
}

/// Escapes into a byte buffer rather than a `String` so multi-byte characters
/// survive: the source is UTF-8 and is appended byte by byte.
fn push_escaped(out: &mut Vec<u8>, text: &[u8]) {
  for &c in text {
    // carriage returns are dropped, matching `HtmlRenderer`, so CRLF sources
    // don't render a stray character at the end of every line
    if c == b'\r' {
      continue;
    }
    match html_escape(c) {
      Some(escape) => out.extend_from_slice(escape.as_bytes()),
      None => out.push(c),
    }
  }
}

/// Renders the highlighter's event stream to one HTML string per source line,
/// wrapping `links` in anchors along the way.
///
/// This replaces `tree_sitter_highlight::HtmlRenderer`, which cannot emit
/// anchors. With an empty `links` it produces byte-identical output (see
/// `matches_upstream_renderer`).
///
/// An anchor never spans a highlight boundary or a line break: a link that
/// covers several highlight spans becomes several anchors pointing at the same
/// href, which keeps the markup well-formed and every part of it clickable.
fn render_lines(
  events: impl Iterator<Item = Result<HighlightEvent, tree_sitter_highlight::Error>>,
  source: &[u8],
  links: &[SourceLink],
) -> Result<Vec<String>, tree_sitter_highlight::Error> {
  fn open_span(out: &mut Vec<u8>, highlight: Highlight) {
    out.extend_from_slice(b"<span ");
    out.extend_from_slice(classes(highlight));
    out.push(b'>');
  }

  let mut lines: Vec<String> = Vec::new();
  let mut line: Vec<u8> = Vec::new();
  let mut highlights: Vec<Highlight> = Vec::new();

  for event in events {
    match event? {
      HighlightEvent::HighlightStart(highlight) => {
        highlights.push(highlight);
        open_span(&mut line, highlight);
      }
      HighlightEvent::HighlightEnd => {
        highlights.pop();
        line.extend_from_slice(b"</span>");
      }
      HighlightEvent::Source { start, end } => {
        let mut pos = start;
        while pos < end {
          // the chunk runs until whichever comes first: the end of the link we
          // are inside, the start of the next one, the end of the event, or a
          // line break
          let link = links.iter().find(|link| link.range.contains(&pos));
          let mut next = end;
          if let Some(link) = link {
            next = next.min(link.range.end);
          } else if let Some(link) =
            links.iter().find(|link| link.range.start > pos)
          {
            next = next.min(link.range.start);
          }
          let chunk_end =
            match source[pos..next].iter().position(|byte| *byte == b'\n') {
              Some(offset) => pos + offset + 1,
              None => next,
            };

          let (text, ends_line) = match source[pos..chunk_end].split_last() {
            Some((b'\n', rest)) => (rest, true),
            _ => (&source[pos..chunk_end], false),
          };

          if let Some(link) = link {
            // `no_color` keeps the syntax highlighting of the specifier; the
            // stylesheet gives these an underline on hover instead
            line.extend_from_slice(
              br#"<a class="no_color specifierLink" href=""#,
            );
            push_escaped(&mut line, link.href.as_bytes());
            line.extend_from_slice(br#"">"#);
          }
          push_escaped(&mut line, text);
          if link.is_some() {
            line.extend_from_slice(b"</a>");
          }

          if ends_line {
            // close every open highlight so each line is standalone markup,
            // then reopen them on the next one
            for _ in &highlights {
              line.extend_from_slice(b"</span>");
            }
            line.push(b'\n');
            lines.push(finish_line(std::mem::take(&mut line)));
            for highlight in &highlights {
              open_span(&mut line, *highlight);
            }
          }

          pos = chunk_end;
        }
      }
    }
  }

  // a source that does not end in a newline still gets a final line; one that
  // does must not gain an empty one
  if !line.is_empty() {
    line.push(b'\n');
    lines.push(finish_line(line));
  }

  Ok(lines)
}

/// The highlighter only ever hands us whole code points and the markup we add
/// is ASCII, so this is infallible in practice; `from_utf8_lossy` keeps a
/// pathological input from taking the request down.
fn finish_line(line: Vec<u8>) -> String {
  match String::from_utf8(line) {
    Ok(line) => line,
    Err(err) => String::from_utf8_lossy(err.as_bytes()).into_owned(),
  }
}

pub fn tree_sitter_language_cb(
  lang: &str,
) -> Option<&'static HighlightConfiguration> {
  for lang in lang.split(',') {
    let cfg = match lang.trim() {
      "js" | "javascript" | "mjs" | "cjs" => tree_sitter_language_javascript(),
      "jsx" => tree_sitter_language_jsx(),
      "ts" | "typescript" | "mts" | "cts" => tree_sitter_language_typescript(),
      "tsx" => tree_sitter_language_tsx(),
      "json" | "jsonc" => tree_sitter_language_json(),
      "css" => tree_sitter_language_css(),
      "md" | "markdown" => tree_sitter_language_markdown(),
      "xml" => tree_sitter_language_xml(),
      "dtd" => tree_sitter_language_dtd(),
      "regex" => tree_sitter_language_regex(),
      "rs" | "rust" => tree_sitter_language_rust(),
      "html" => tree_sitter_language_html(),
      "sh" | "bash" => tree_sitter_language_bash(),
      "toml" => tree_sitter_language_toml(),
      "yaml" | "yml" => tree_sitter_language_yaml(),
      "c" | "h" => tree_sitter_language_c(),
      _ => continue,
    };
    return Some(cfg);
  }
  None
}

pub fn tree_sitter_language_javascript() -> &'static HighlightConfiguration {
  static CONFIG: OnceLock<HighlightConfiguration> = OnceLock::new();
  CONFIG.get_or_init(|| {
    let mut config = HighlightConfiguration::new(
      tree_sitter_javascript::language(),
      "javascript",
      tree_sitter_javascript::HIGHLIGHT_QUERY,
      tree_sitter_javascript::INJECTIONS_QUERY,
      tree_sitter_javascript::LOCALS_QUERY,
    )
    .expect("failed to initialize tree_sitter_javascript highlighter");
    config.configure(CAPTURE_NAMES);
    config
  })
}

pub fn tree_sitter_language_jsx() -> &'static HighlightConfiguration {
  static CONFIG: OnceLock<HighlightConfiguration> = OnceLock::new();
  CONFIG.get_or_init(|| {
    let mut config = HighlightConfiguration::new(
      tree_sitter_javascript::language(),
      "jsx",
      format!(
        "{} {}",
        tree_sitter_javascript::HIGHLIGHT_QUERY,
        tree_sitter_javascript::JSX_HIGHLIGHT_QUERY
      )
      .leak(),
      tree_sitter_javascript::INJECTIONS_QUERY,
      tree_sitter_javascript::LOCALS_QUERY,
    )
    .expect("failed to initialize tree_sitter_javascript highlighter");
    config.configure(CAPTURE_NAMES);
    config
  })
}

pub fn tree_sitter_language_typescript() -> &'static HighlightConfiguration {
  static CONFIG: OnceLock<HighlightConfiguration> = OnceLock::new();
  CONFIG.get_or_init(|| {
    let mut config = HighlightConfiguration::new(
      tree_sitter_typescript::language_typescript(),
      "typescript",
      format!(
        "{} {}",
        tree_sitter_javascript::HIGHLIGHT_QUERY,
        tree_sitter_typescript::HIGHLIGHTS_QUERY
      )
      .leak(),
      tree_sitter_javascript::INJECTIONS_QUERY,
      format!(
        "{} {}",
        tree_sitter_javascript::LOCALS_QUERY,
        tree_sitter_typescript::LOCALS_QUERY
      )
      .leak(),
    )
    .expect("failed to initialize tree_sitter_typescript highlighter");
    config.configure(CAPTURE_NAMES);
    config
  })
}

pub fn tree_sitter_language_tsx() -> &'static HighlightConfiguration {
  static CONFIG: OnceLock<HighlightConfiguration> = OnceLock::new();
  CONFIG.get_or_init(|| {
    let mut config = HighlightConfiguration::new(
      tree_sitter_typescript::language_tsx(),
      "tsx",
      format!(
        "{} {} {}",
        tree_sitter_javascript::HIGHLIGHT_QUERY,
        tree_sitter_javascript::JSX_HIGHLIGHT_QUERY,
        tree_sitter_typescript::HIGHLIGHTS_QUERY,
      )
      .leak(),
      tree_sitter_javascript::INJECTIONS_QUERY,
      format!(
        "{} {}",
        tree_sitter_javascript::LOCALS_QUERY,
        tree_sitter_typescript::LOCALS_QUERY
      )
      .leak(),
    )
    .expect("failed to initialize tree_sitter_typescript highlighter");
    config.configure(CAPTURE_NAMES);
    config
  })
}

fn tree_sitter_language_json() -> &'static HighlightConfiguration {
  static CONFIG: OnceLock<HighlightConfiguration> = OnceLock::new();
  CONFIG.get_or_init(|| {
    let mut config = HighlightConfiguration::new(
      tree_sitter_json::language(),
      "json",
      tree_sitter_json::HIGHLIGHTS_QUERY,
      "",
      "",
    )
    .expect("failed to initialize tree_sitter_json highlighter");
    config.configure(CAPTURE_NAMES);
    config
  })
}

fn tree_sitter_language_css() -> &'static HighlightConfiguration {
  static CONFIG: OnceLock<HighlightConfiguration> = OnceLock::new();
  CONFIG.get_or_init(|| {
    let mut config = HighlightConfiguration::new(
      tree_sitter_css::language(),
      "css",
      tree_sitter_css::HIGHLIGHTS_QUERY,
      "",
      "",
    )
    .expect("failed to initialize tree_sitter_css highlighter");
    config.configure(CAPTURE_NAMES);
    config
  })
}

fn tree_sitter_language_markdown() -> &'static HighlightConfiguration {
  static CONFIG: OnceLock<HighlightConfiguration> = OnceLock::new();
  CONFIG.get_or_init(|| {
    let mut config = HighlightConfiguration::new(
      tree_sitter_md::language(),
      "markdown",
      tree_sitter_md::HIGHLIGHT_QUERY_BLOCK,
      tree_sitter_md::INJECTION_QUERY_BLOCK,
      "",
    )
    .expect("failed to initialize tree_sitter_md highlighter");
    config.configure(CAPTURE_NAMES);
    config
  })
}

fn tree_sitter_language_xml() -> &'static HighlightConfiguration {
  static CONFIG: OnceLock<HighlightConfiguration> = OnceLock::new();
  CONFIG.get_or_init(|| {
    let mut config = HighlightConfiguration::new(
      tree_sitter_xml::language_xml(),
      "xml",
      tree_sitter_xml::XML_HIGHLIGHT_QUERY,
      "",
      "",
    )
    .expect("failed to initialize tree_sitter_xml highlighter");
    config.configure(CAPTURE_NAMES);
    config
  })
}

fn tree_sitter_language_dtd() -> &'static HighlightConfiguration {
  static CONFIG: OnceLock<HighlightConfiguration> = OnceLock::new();
  CONFIG.get_or_init(|| {
    let mut config = HighlightConfiguration::new(
      tree_sitter_xml::language_dtd(),
      "dtd",
      tree_sitter_xml::DTD_HIGHLIGHT_QUERY,
      "",
      "",
    )
    .expect("failed to initialize tree_sitter_dtd highlighter");
    config.configure(CAPTURE_NAMES);
    config
  })
}

fn tree_sitter_language_regex() -> &'static HighlightConfiguration {
  static CONFIG: OnceLock<HighlightConfiguration> = OnceLock::new();
  CONFIG.get_or_init(|| {
    let mut config = HighlightConfiguration::new(
      tree_sitter_regex::language(),
      "regex",
      tree_sitter_regex::HIGHLIGHTS_QUERY,
      "",
      "",
    )
    .expect("failed to initialize tree_sitter_regex highlighter");
    config.configure(CAPTURE_NAMES);
    config
  })
}

fn tree_sitter_language_rust() -> &'static HighlightConfiguration {
  static CONFIG: OnceLock<HighlightConfiguration> = OnceLock::new();
  CONFIG.get_or_init(|| {
    let mut config = HighlightConfiguration::new(
      tree_sitter_rust::language(),
      "rust",
      tree_sitter_rust::HIGHLIGHTS_QUERY,
      tree_sitter_rust::INJECTIONS_QUERY,
      "",
    )
    .expect("failed to initialize tree_sitter_rust highlighter");
    config.configure(CAPTURE_NAMES);
    config
  })
}

fn tree_sitter_language_html() -> &'static HighlightConfiguration {
  static CONFIG: OnceLock<HighlightConfiguration> = OnceLock::new();
  CONFIG.get_or_init(|| {
    let mut config = HighlightConfiguration::new(
      tree_sitter_html::language(),
      "html",
      tree_sitter_html::HIGHLIGHTS_QUERY,
      tree_sitter_html::INJECTIONS_QUERY,
      "",
    )
    .expect("failed to initialize tree_sitter_html highlighter");
    config.configure(CAPTURE_NAMES);
    config
  })
}

fn tree_sitter_language_bash() -> &'static HighlightConfiguration {
  static CONFIG: OnceLock<HighlightConfiguration> = OnceLock::new();
  CONFIG.get_or_init(|| {
    let mut config = HighlightConfiguration::new(
      tree_sitter_bash::language(),
      "bash",
      tree_sitter_bash::HIGHLIGHT_QUERY,
      "",
      "",
    )
    .expect("failed to initialize tree_sitter_bash highlighter");
    config.configure(CAPTURE_NAMES);
    config
  })
}

fn tree_sitter_language_toml() -> &'static HighlightConfiguration {
  static CONFIG: OnceLock<HighlightConfiguration> = OnceLock::new();
  CONFIG.get_or_init(|| {
    let mut config = HighlightConfiguration::new(
      tree_sitter_toml_ng::language(),
      "toml",
      tree_sitter_toml_ng::HIGHLIGHTS_QUERY,
      "",
      "",
    )
    .expect("failed to initialize tree_sitter_toml highlighter");
    config.configure(CAPTURE_NAMES);
    config
  })
}

fn tree_sitter_language_yaml() -> &'static HighlightConfiguration {
  static CONFIG: OnceLock<HighlightConfiguration> = OnceLock::new();
  CONFIG.get_or_init(|| {
    let mut config = HighlightConfiguration::new(
      tree_sitter_yaml::language(),
      "yaml",
      tree_sitter_yaml::HIGHLIGHTS_QUERY,
      "",
      "",
    )
    .expect("failed to initialize tree_sitter_yaml highlighter");
    config.configure(CAPTURE_NAMES);
    config
  })
}

fn tree_sitter_language_c() -> &'static HighlightConfiguration {
  static CONFIG: OnceLock<HighlightConfiguration> = OnceLock::new();
  CONFIG.get_or_init(|| {
    let mut config = HighlightConfiguration::new(
      tree_sitter_c::language(),
      "c",
      tree_sitter_c::HIGHLIGHT_QUERY,
      "",
      "",
    )
    .expect("failed to initialize tree_sitter_c highlighter");
    config.configure(CAPTURE_NAMES);
    config
  })
}

#[cfg(test)]
mod tests {
  use super::*;
  use comrak::adapters::SyntaxHighlighterAdapter;

  fn highlight(lang: &str, code: &str) -> String {
    let adapter = ComrakAdapter::new(false);
    let mut out = Vec::new();
    adapter
      .write_highlighted(&mut out, Some(lang), code)
      .unwrap();
    String::from_utf8(out).unwrap()
  }

  #[test]
  fn highlights_added_languages() {
    for (lang, code) in [
      ("toml", "[package]\nname = \"demo\"\n"),
      ("yaml", "key: value\nlist:\n  - a\n"),
      ("yml", "key: value\n"),
      ("c", "int main(void) { return 0; }\n"),
      ("mjs", "export const x = 1;\n"),
      ("mts", "export const x: number = 1;\n"),
    ] {
      assert!(
        tree_sitter_language_cb(lang).is_some(),
        "no highlighter for {lang}"
      );
      let html = highlight(lang, code);
      assert!(html.contains("<span"), "{lang} did not highlight: {html}");
    }
  }

  fn rendered_lines(
    lang: &str,
    code: &str,
    links: &[SourceLink],
  ) -> Vec<String> {
    let config = tree_sitter_language_cb(lang).unwrap();
    let source = code.as_bytes();
    let mut highlighter = tree_sitter_highlight::Highlighter::new();
    #[allow(clippy::redundant_closure)]
    let events = highlighter
      .highlight(config, source, None, |e| tree_sitter_language_cb(e))
      .unwrap();
    render_lines(events, source, links).unwrap()
  }

  fn upstream_lines(lang: &str, code: &str) -> Vec<String> {
    let config = tree_sitter_language_cb(lang).unwrap();
    let source = code.as_bytes();
    let mut highlighter = tree_sitter_highlight::Highlighter::new();
    #[allow(clippy::redundant_closure)]
    let events = highlighter
      .highlight(config, source, None, |e| tree_sitter_language_cb(e))
      .unwrap();
    let mut renderer = tree_sitter_highlight::HtmlRenderer::new();
    renderer
      .render(events, source, &|highlight| classes(highlight))
      .unwrap();
    renderer.lines().map(|line| line.to_string()).collect()
  }

  /// Without links, the hand-rolled renderer must be a drop-in replacement for
  /// the upstream one, which every markdown code fence still goes through.
  #[test]
  fn matches_upstream_renderer() {
    for (lang, code) in [
      (
        "ts",
        "import { a } from \"./a.ts\";\nexport const x: number = 1;\n",
      ),
      // no trailing newline
      ("ts", "const x = 1;"),
      // CRLF, blank lines, and a string spanning a line break
      ("ts", "const a = 1;\r\n\r\nconst b = `multi\nline`;\r\n"),
      // characters that need escaping, and non-ASCII ones that must not be
      // mangled by the byte-wise escaping
      ("ts", "const s = \"<a & b>\";\nconst β = 'π';\n"),
      ("md", "# Title\n\nSome *text*.\n"),
      ("json", "{\n  \"a\": [1, 2]\n}\n"),
    ] {
      assert_eq!(
        rendered_lines(lang, code, &[]),
        upstream_lines(lang, code),
        "mismatch for {lang}: {code:?}"
      );
    }
  }

  #[test]
  fn wraps_linked_ranges_in_anchors() {
    let code = "import { a } from \"./a.ts\";\n";
    let start = code.find("\"./a.ts\"").unwrap();
    let links = vec![SourceLink {
      range: start..start + "\"./a.ts\"".len(),
      href: "/@scope/pkg/1.0.0/a.ts".to_string(),
    }];

    let lines = rendered_lines("ts", code, &links);
    let html = lines.join("");

    assert!(
      html.contains(
        r#"<a class="no_color specifierLink" href="/@scope/pkg/1.0.0/a.ts">"#
      ),
      "{html}"
    );
    // the quotes are part of the link text, and are still escaped
    assert!(html.contains("&quot;./a.ts&quot;</a>"), "{html}");
    // the rest of the line is untouched
    assert_eq!(html.matches("<a ").count(), 1, "{html}");
    assert_eq!(html.matches("</a>").count(), 1, "{html}");
  }

  /// A link whose range covers several highlight spans becomes one anchor per
  /// span, so the markup stays properly nested.
  #[test]
  fn anchors_never_cross_a_highlight_boundary() {
    let code = "import { a } from \"./a.ts\";\n";
    let links = vec![SourceLink {
      // deliberately covers the whole line, spanning keywords and strings
      range: 0..code.len() - 1,
      href: "/somewhere".to_string(),
    }];

    let html = rendered_lines("ts", code, &links).join("");
    assert_eq!(
      html.matches("<a ").count(),
      html.matches("</a>").count(),
      "{html}"
    );
    // every anchor closes before the span it sits in does
    let mut depth = 0i32;
    for token in html.split('<') {
      if token.starts_with("a ") {
        depth += 1;
      } else if token.starts_with("/a>") {
        depth -= 1;
      } else if token.starts_with("/span>") {
        assert_eq!(depth, 0, "anchor left open across a span: {html}");
      }
      assert!(depth >= 0, "{html}");
    }
    assert_eq!(depth, 0, "{html}");
  }

  /// A specifier on the last line of a file without a trailing newline still
  /// gets its anchor, and no phantom line is emitted.
  #[test]
  fn links_on_a_final_line_without_a_newline() {
    let code = "import \"./a.ts\";";
    let start = code.find("\"./a.ts\"").unwrap();
    let links = vec![SourceLink {
      range: start..start + "\"./a.ts\"".len(),
      href: "/a".to_string(),
    }];

    let lines = rendered_lines("ts", code, &links);
    assert_eq!(lines.len(), 1, "{lines:?}");
    assert!(lines[0].contains(r#"href="/a""#), "{lines:?}");
    assert!(lines[0].ends_with('\n'), "{lines:?}");
  }
}
