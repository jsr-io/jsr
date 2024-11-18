// Copyright 2024 the JSR authors. All rights reserved. MIT license.
use std::collections::HashMap;
use std::io::Write;
use std::sync::OnceLock;

use tree_sitter_highlight::Highlight;
use tree_sitter_highlight::HighlightConfiguration;

pub struct ComrakAdapter {
  pub show_line_numbers: bool,
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
        Ok(highlighter) => {
          let mut renderer = tree_sitter_highlight::HtmlRenderer::new();
          match renderer
            .render(highlighter, source, &|highlight| classes(highlight))
          {
            Ok(()) => {
              let mut line_numbers = String::new();
              let mut lines = String::new();

              for (i, line) in renderer.lines().enumerate() {
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
                  r##"<div class="lineNumbers">{line_numbers}</div><div class="grow overflow-x-auto lineNumbersHighlight">{lines}</div>"##
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
        .push_str(" !flex gap-2");
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

pub fn tree_sitter_language_cb(
  lang: &str,
) -> Option<&'static HighlightConfiguration> {
  for lang in lang.split(',') {
    let cfg = match lang.trim() {
      "js" | "javascript" => tree_sitter_language_javascript(),
      "jsx" => tree_sitter_language_jsx(),
      "ts" | "typescript" => tree_sitter_language_typescript(),
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
