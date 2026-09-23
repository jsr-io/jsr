// Copyright 2024 the JSR authors. All rights reserved. MIT license.
//! Turns the import/export specifiers recorded in a version's module graph
//! into clickable links in the file view (jsr-io/jsr#17).
//!
//! Every published version stores a `moduleGraph2` in its metadata, and each
//! entry already carries the source range of every specifier it mentions —
//! static and dynamic imports, `export ... from`, triple-slash references, the
//! JSX import source pragma and JSDoc type imports. So the file view does not
//! have to re-parse anything: it maps those ranges onto byte offsets in the
//! file it is about to highlight and hands them to the highlighter.

use std::collections::HashSet;

use deno_graph::Position;
use deno_graph::PositionRange;
use deno_graph::analysis::DependencyDescriptor;
use deno_graph::analysis::DynamicArgument;
use deno_graph::analysis::ModuleInfo;
use deno_graph::analysis::TypeScriptReference;
use deno_semver::RangeSetOrTag;
use url::Url;

use crate::ids::PackageName;
use crate::ids::ScopeName;
use crate::ids::Version;
use crate::tree_sitter::SourceLink;

/// Everything needed to turn a specifier into a URL on this site.
pub struct LinkContext<'a> {
  pub scope: &'a ScopeName,
  pub package: &'a PackageName,
  pub version: &'a Version,
  /// Path of the file being viewed, e.g. `/src/mod.ts`.
  pub current_path: &'a str,
  /// Paths of the files in this version, used so a relative specifier only
  /// becomes a link when it actually resolves to a file we can show.
  pub files: &'a HashSet<String>,
  pub registry_url: &'a str,
}

/// Collects the linkable specifiers of one module, sorted by position and with
/// overlaps removed so the highlighter can walk them in one pass.
pub fn specifier_links(
  module_info: &ModuleInfo,
  source: &str,
  ctx: &LinkContext,
) -> Vec<SourceLink> {
  let lines = LineIndex::new(source);
  let mut links = Vec::new();

  let mut push = |specifier: &str, range: &PositionRange| {
    let Some(href) = resolve_href(specifier, ctx) else {
      return;
    };
    let Some(range) = lines.byte_range(range) else {
      return;
    };
    if range.is_empty() {
      return;
    }
    links.push(SourceLink { range, href });
  };

  for dependency in &module_info.dependencies {
    match dependency {
      DependencyDescriptor::Static(dependency) => {
        push(&dependency.specifier, &dependency.specifier_range);
        if let Some(types) = &dependency.types_specifier {
          push(&types.text, &types.range);
        }
      }
      DependencyDescriptor::Dynamic(dependency) => {
        // only a dynamic import with a plain string argument has a specifier
        // to link; templates and computed expressions do not
        if let DynamicArgument::String(specifier) = &dependency.argument {
          push(specifier, &dependency.argument_range);
        }
        if let Some(types) = &dependency.types_specifier {
          push(&types.text, &types.range);
        }
      }
    }
  }

  for reference in &module_info.ts_references {
    let specifier = match reference {
      TypeScriptReference::Path(specifier) => specifier,
      TypeScriptReference::Types { specifier, .. } => specifier,
    };
    push(&specifier.text, &specifier.range);
  }

  for specifier in [
    module_info.self_types_specifier.as_ref(),
    module_info.jsx_import_source.as_ref(),
    module_info.jsx_import_source_types.as_ref(),
  ]
  .into_iter()
  .flatten()
  {
    push(&specifier.text, &specifier.range);
  }

  for import in &module_info.jsdoc_imports {
    push(&import.specifier.text, &import.specifier.range);
  }

  links.sort_by_key(|link| link.range.start);
  // ranges from different sources can name the same specifier (a `@ts-types`
  // override sitting on the import it overrides, say); keep the first
  links.dedup_by(|later, earlier| later.range.start < earlier.range.end);
  links
}

/// Whether a file is one the module graph could have an entry for. Saves
/// fetching the version metadata when viewing a README or a JSON file.
pub fn is_module_path(path: &str) -> bool {
  matches!(
    path.rsplit_once('.').map(|(_, extension)| extension),
    Some("js" | "mjs" | "cjs" | "jsx" | "ts" | "mts" | "cts" | "tsx")
  )
}

fn resolve_href(specifier: &str, ctx: &LinkContext) -> Option<String> {
  if specifier.starts_with('.') {
    return resolve_relative_href(specifier, ctx);
  }

  let url = Url::parse(specifier).ok()?;
  match url.scheme() {
    "node" => Some(format!("https://nodejs.org/api/{}.html", url.path())),
    "npm" => {
      let reference =
        deno_semver::npm::NpmPackageReqReference::from_str(specifier).ok()?;
      let req = reference.req();
      Some(format!(
        "https://www.npmjs.com/package/{}{}",
        req.name,
        match req.version_req.inner() {
          RangeSetOrTag::RangeSet(_) => String::new(),
          RangeSetOrTag::Tag(tag) => format!("/v/{tag}"),
        },
      ))
    }
    "jsr" => {
      let reference =
        deno_semver::jsr::JsrPackageReqReference::from_str(specifier).ok()?;
      let req = reference.req();

      // link to the exact version when the specifier pins one, so the reader
      // lands on the code that is actually imported
      let version = req
        .version_req
        .range()
        .and_then(|range| Version::new(&range.to_string()).ok())
        .map(|version| format!("@{version}"))
        .unwrap_or_default();

      Some(format!("/{}{version}", req.name))
    }
    "http" | "https" if specifier.starts_with(ctx.registry_url) => {
      // a direct registry URL, e.g. https://jsr.io/@scope/name/1.2.3/mod.ts
      let parts = url.path().splitn(4, '/').collect::<Vec<_>>();
      let [_, scope, package, rest] = parts[..] else {
        return None;
      };
      Some(format!("/{scope}/{package}@{rest}"))
    }
    "http" | "https" => Some(url.to_string()),
    // `bun:`, `cloudflare:`, `virtual:` and friends have nothing to point at
    _ => None,
  }
}

fn resolve_relative_href(specifier: &str, ctx: &LinkContext) -> Option<String> {
  let base = Url::parse(&format!("file://{}", ctx.current_path)).ok()?;
  let resolved = base.join(specifier).ok()?;
  let path = resolved.path();

  // don't offer a link the file view would 404 on: a specifier can point
  // outside the package, or rely on resolution jsr does not replay here
  if !ctx.files.contains(path) {
    return None;
  }

  Some(format!(
    "/@{}/{}/{}{path}",
    ctx.scope, ctx.package, ctx.version
  ))
}

/// Maps the module graph's `(line, character)` positions onto byte offsets.
///
/// `character` counts characters, not bytes and not UTF-16 units, so lines
/// with multi-byte characters before a specifier are walked by character.
struct LineIndex<'a> {
  source: &'a str,
  starts: Vec<usize>,
}

impl<'a> LineIndex<'a> {
  fn new(source: &'a str) -> Self {
    let mut starts = vec![0];
    starts.extend(source.match_indices('\n').map(|(index, _)| index + 1));
    Self { source, starts }
  }

  fn byte_offset(&self, position: &Position) -> Option<usize> {
    let start = *self.starts.get(position.line)?;
    let end = self
      .starts
      .get(position.line + 1)
      .copied()
      .unwrap_or(self.source.len());
    let line = &self.source[start..end];

    let offset = line
      .char_indices()
      .nth(position.character)
      .map(|(offset, _)| offset)
      .unwrap_or(line.len());
    Some(start + offset)
  }

  fn byte_range(
    &self,
    range: &PositionRange,
  ) -> Option<std::ops::Range<usize>> {
    let start = self.byte_offset(&range.start)?;
    let end = self.byte_offset(&range.end)?;
    (start <= end && end <= self.source.len()).then_some(start..end)
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  /// Resolves a specifier as if viewing `/src/mod.ts` of `@scope/pkg@1.2.3`,
  /// with `files` as the package's file list.
  fn resolve(specifier: &str, files: &[&str]) -> Option<String> {
    let scope = ScopeName::new("scope".to_string()).unwrap();
    let package = PackageName::new("pkg".to_string()).unwrap();
    let version = Version::new("1.2.3").unwrap();
    let files: HashSet<String> =
      files.iter().map(|path| path.to_string()).collect();

    resolve_href(
      specifier,
      &LinkContext {
        scope: &scope,
        package: &package,
        version: &version,
        current_path: "/src/mod.ts",
        files: &files,
        registry_url: "https://jsr.io",
      },
    )
  }

  const FILES: &[&str] = &["/src/mod.ts", "/src/util.ts", "/other.ts"];

  #[test]
  fn line_index_handles_multi_byte_characters() {
    // `β` is two bytes, so the character index and the byte offset differ
    let source = "const β = 1;\nimport x from \"./a.ts\";\n";
    let index = LineIndex::new(source);

    assert_eq!(index.byte_offset(&Position::new(0, 6)), Some(6));
    // after the two-byte β, character 7 is byte 8
    assert_eq!(index.byte_offset(&Position::new(0, 7)), Some(8));
    // line 1 starts right after the newline
    assert_eq!(index.byte_offset(&Position::new(1, 0)), Some(14));
    assert_eq!(
      &source[index.byte_offset(&Position::new(1, 14)).unwrap()..],
      "\"./a.ts\";\n"
    );
    // out of range lines resolve to nothing rather than panicking
    assert_eq!(index.byte_offset(&Position::new(9, 0)), None);
  }

  #[test]
  fn relative_specifiers_link_to_the_file_view() {
    assert_eq!(
      resolve("./util.ts", FILES).as_deref(),
      Some("/@scope/pkg/1.2.3/src/util.ts")
    );
    assert_eq!(
      resolve("../other.ts", FILES).as_deref(),
      Some("/@scope/pkg/1.2.3/other.ts")
    );
    // a specifier that does not name a file in the package is left alone,
    // rather than becoming a link to a 404
    assert_eq!(resolve("./missing.ts", FILES), None);
    assert_eq!(resolve("../../escape.ts", FILES), None);
  }

  #[test]
  fn external_specifiers_link_off_site() {
    assert_eq!(
      resolve("node:fs", FILES).as_deref(),
      Some("https://nodejs.org/api/fs.html")
    );
    assert_eq!(
      resolve("npm:chalk@5", FILES).as_deref(),
      Some("https://www.npmjs.com/package/chalk")
    );
    assert_eq!(
      resolve("jsr:@std/assert@^1.0.0", FILES).as_deref(),
      Some("/@std/assert")
    );
    assert_eq!(
      resolve("jsr:@std/assert@1.0.2", FILES).as_deref(),
      Some("/@std/assert@1.0.2")
    );
    assert_eq!(
      resolve("jsr:@std/assert@1.0.2/equals", FILES).as_deref(),
      Some("/@std/assert@1.0.2")
    );
    assert_eq!(
      resolve("https://deno.land/x/foo/mod.ts", FILES).as_deref(),
      Some("https://deno.land/x/foo/mod.ts")
    );
    // registry URLs point back into the site
    assert_eq!(
      resolve("https://jsr.io/@std/assert/1.0.2/mod.ts", FILES).as_deref(),
      Some("/@std/assert@1.0.2/mod.ts")
    );
    // schemes with nowhere to go stay plain text
    assert_eq!(resolve("bun:test", FILES), None);
    assert_eq!(resolve("cloudflare:workers", FILES), None);
    assert_eq!(resolve("lodash", FILES), None);
  }

  #[test]
  fn is_module_path_covers_the_view() {
    for path in [
      "/mod.ts",
      "/mod.tsx",
      "/a.mts",
      "/a.cts",
      "/a.js",
      "/a.mjs",
      "/a.cjs",
      "/a.jsx",
      "/types.d.ts",
    ] {
      assert!(is_module_path(path), "{path}");
    }
    for path in ["/README.md", "/deno.json", "/a.wasm", "/a.css"] {
      assert!(!is_module_path(path), "{path}");
    }
  }
}
