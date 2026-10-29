// Copyright 2024 the JSR authors. All rights reserved. MIT license.
use std::borrow::Cow;
use std::collections::HashMap;

use base64::Engine;
use deno_ast::MediaType;
use deno_ast::ParsedSource;
use deno_ast::SourceTextInfo;
use deno_ast::TextChange;
use deno_ast::TranspileOptions;
use deno_ast::apply_text_changes;
use deno_ast::swc::ecma_visit::VisitWith;
use deno_graph::JsModule;
use deno_graph::ModuleGraph;
use deno_graph::ModuleSpecifier;
use deno_graph::PositionRange;
use deno_graph::Resolution;
use deno_graph::analysis::DependencyDescriptor;
use deno_graph::analysis::ModuleInfo;
use deno_graph::ast::EsParser;
use deno_graph::ast::ParseOptions;
use deno_graph::ast::ParserModuleAnalyzer;
use deno_semver::Version;
use deno_semver::package::PackageReqReference;
use flate2::Compression;
use flate2::write::GzEncoder;
use futures::StreamExt;
use futures::TryStreamExt;
use indexmap::IndexMap;
use serde::Serialize;
use sha2::Digest;

use crate::emit::transpile_to_dts;
use crate::emit::transpile_to_js;
use crate::import_transform::ImportMetaResolveCollector;
use crate::specifiers::Extension;
use crate::specifiers::RewriteKind;
use crate::specifiers::SpecifierRewriter;
use crate::specifiers::follow_specifier;
use crate::specifiers::jsr_to_npm_package_name;
use crate::specifiers::relative_import_specifier;
use crate::specifiers::rewrite_file_specifier;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum DependencyKind {
  Jsr,
  Npm,
}

pub struct NpmTarball {
  /// The gzipped tarball contents.
  pub tarball: Vec<u8>,
  /// The hex encoded sha1 hash of the gzipped tarball.
  pub sha1: String,
  /// The base64 encoded sha512 hash of the gzipped tarball.
  pub sha512: String,
  /// The files in the tarball, sorted by path.
  pub entries: Vec<NpmTarballEntry>,
}

pub struct NpmTarballEntry {
  /// The path of the file in the package, like `/mod.js`.
  pub path: String,
  pub size: usize,
}

/// The files of a package, by their path in the package, like `/mod.ts`.
#[allow(async_fn_in_trait)]
pub trait PackageFiles {
  fn paths(&self) -> impl Iterator<Item = &str>;

  /// Reads a file. It's only called for files that aren't replaced by
  /// generated ones.
  async fn read(&self, path: &str) -> Result<Vec<u8>, anyhow::Error>;
}

impl PackageFiles for HashMap<String, Vec<u8>> {
  fn paths(&self) -> impl Iterator<Item = &str> {
    self.keys().map(String::as_str)
  }

  async fn read(&self, path: &str) -> Result<Vec<u8>, anyhow::Error> {
    self
      .get(path)
      .cloned()
      .ok_or_else(|| anyhow::anyhow!("file missing: {path}"))
  }
}

pub struct NpmPackageJsonOptions<'a> {
  /// The name of the npm package, like `@jsr/std__fs`.
  pub name: &'a str,
  pub version: &'a Version,
  pub homepage: Option<String>,
  pub license: Option<String>,
  /// Written to `_jsr_revision`.
  pub jsr_revision: Option<u32>,
}

pub struct NpmTarballOptions<'a, Exports, Deps, Files> {
  pub graph: &'a ModuleGraph,
  /// Parses the modules in the graph. Pass a capturing parser to reuse the
  /// sources parsed while building the graph.
  pub parser: &'a dyn EsParser,
  pub package_json: NpmPackageJsonOptions<'a>,
  /// The exports of the package, like `(".", "./mod.ts")`.
  pub exports: Exports,
  pub files: &'a Files,
  pub dependencies: Deps,
  pub transpile_options: &'a TranspileOptions,
  /// Whether to write source maps for the generated JavaScript and
  /// declaration files.
  pub source_maps: bool,
}

#[derive(Debug, Serialize)]
struct NpmExportConditions {
  #[serde(skip_serializing_if = "Option::is_none")]
  types: Option<String>,
  #[serde(skip_serializing_if = "Option::is_none")]
  default: Option<String>,
}

#[derive(Debug, Serialize)]
struct NpmPackageJson<'a> {
  name: &'a str,
  version: String,
  #[serde(skip_serializing_if = "Option::is_none")]
  homepage: Option<String>,
  #[serde(skip_serializing_if = "Option::is_none")]
  license: Option<String>,

  #[serde(rename = "type")]
  module_type: &'static str,
  dependencies: IndexMap<String, String>,
  exports: IndexMap<String, NpmExportConditions>,

  #[serde(rename = "_jsr_revision", skip_serializing_if = "Option::is_none")]
  revision: Option<u32>,
}

pub async fn create_npm_tarball<'a>(
  opts: NpmTarballOptions<
    'a,
    impl Iterator<Item = (&'a str, &'a str)>,
    impl Iterator<Item = &'a (DependencyKind, PackageReqReference)>,
    impl PackageFiles,
  >,
) -> Result<NpmTarball, anyhow::Error> {
  let NpmTarballOptions {
    graph,
    parser,
    package_json,
    exports,
    files,
    dependencies,
    transpile_options,
    source_maps,
  } = opts;

  let npm_dependencies =
    create_npm_dependencies(dependencies.map(Cow::Borrowed))?;

  let mut package_files = IndexMap::new();
  let mut to_be_rewritten = vec![];

  // Mapping of original specifiers in the module graph to where one can find
  // the source code or declarations for that module in the tarball, if it
  // differs from the original specifier.
  let mut source_rewrites = HashMap::<&ModuleSpecifier, ModuleSpecifier>::new();
  let mut declaration_rewrites =
    HashMap::<&ModuleSpecifier, ModuleSpecifier>::new();

  for module in graph.modules() {
    if module.specifier().scheme() != "file" {
      continue;
    };

    let Some(js) = module.js() else { continue };

    match js.media_type {
      MediaType::JavaScript | MediaType::Mjs => {
        if let Some(types_dep) = &js.maybe_types_dependency
          && let Resolution::Ok(resolved) = &types_dep.dependency
        {
          declaration_rewrites
            .insert(module.specifier(), resolved.specifier.clone());
        }
      }
      MediaType::Jsx => {
        let source_specifier =
          rewrite_file_specifier(module.specifier(), "", Extension::Js);
        if let Some(source_specifier) = source_specifier {
          source_rewrites.insert(module.specifier(), source_specifier);
        }

        if let Some(types_dep) = &js.maybe_types_dependency
          && let Resolution::Ok(resolved) = &types_dep.dependency
        {
          declaration_rewrites
            .insert(module.specifier(), resolved.specifier.clone());
        }
      }
      MediaType::Dts | MediaType::Dmts => {
        // no extra work needed for these, as they can not have type dependencies
      }
      MediaType::TypeScript | MediaType::Mts | MediaType::Tsx => {
        let source_specifier =
          rewrite_file_specifier(module.specifier(), "", Extension::Js);
        if let Some(source_specifier) = source_specifier.clone() {
          source_rewrites.insert(module.specifier(), source_specifier);
        }

        if js.fast_check_module().is_some() {
          let declaration_specifier = rewrite_file_specifier(
            module.specifier(),
            "/_dist",
            Extension::Dts,
          );
          if let Some(declaration_specifier) = declaration_specifier {
            declaration_rewrites
              .insert(module.specifier(), declaration_specifier);
          }
        } else if let Some(source_specifier) = source_specifier {
          declaration_rewrites.insert(module.specifier(), source_specifier);
        }
      }
      _ => {}
    }

    to_be_rewritten.push(js);
  }

  for js in to_be_rewritten {
    match js.media_type {
      MediaType::JavaScript | MediaType::Mjs => {
        let parsed_source = parse(parser, js)?;
        let module_info = ParserModuleAnalyzer::module_info(&parsed_source);
        let specifier_rewriter = SpecifierRewriter {
          base_specifier: &js.specifier,
          module_specifier: &js.specifier,
          source_rewrites: &source_rewrites,
          declaration_rewrites: &declaration_rewrites,
          dependencies: &js.dependencies,
        };
        let rewritten = rewrite_specifiers(
          &parsed_source,
          &module_info,
          specifier_rewriter,
          RewriteKind::Source,
        );
        package_files
          .insert(js.specifier.path().to_owned(), rewritten.into_bytes());
      }
      MediaType::Dts | MediaType::Dmts => {
        let parsed_source = parse(parser, js)?;
        let module_info = ParserModuleAnalyzer::module_info(&parsed_source);
        let specifier_rewriter = SpecifierRewriter {
          base_specifier: &js.specifier,
          module_specifier: &js.specifier,
          source_rewrites: &source_rewrites,
          declaration_rewrites: &declaration_rewrites,
          dependencies: &js.dependencies,
        };
        let rewritten = rewrite_specifiers(
          &parsed_source,
          &module_info,
          specifier_rewriter,
          RewriteKind::Declaration,
        );
        package_files
          .insert(js.specifier.path().to_owned(), rewritten.into_bytes());
      }
      MediaType::Jsx => {
        let parsed_source = parse(parser, js)?;
        let source_target = source_rewrites.get(&js.specifier).unwrap();
        let specifier_rewriter = SpecifierRewriter {
          base_specifier: source_target,
          module_specifier: &js.specifier,
          source_rewrites: &source_rewrites,
          declaration_rewrites: &declaration_rewrites,
          dependencies: &js.dependencies,
        };
        let (source, source_map) = transpile_to_js(
          &parsed_source,
          specifier_rewriter,
          source_target,
          transpile_options,
          source_maps,
        )?;
        package_files.insert(source_target.path().to_owned(), source);
        if let Some(source_map) = source_map {
          package_files
            .insert(format!("{}.map", source_target.path()), source_map);
        }
      }
      MediaType::TypeScript | MediaType::Mts | MediaType::Tsx => {
        let parsed_source = parse(parser, js)?;
        let module_info = ParserModuleAnalyzer::module_info(&parsed_source);
        let specifier_rewriter = SpecifierRewriter {
          base_specifier: &js.specifier,
          module_specifier: &js.specifier,
          source_rewrites: &source_rewrites,
          declaration_rewrites: &declaration_rewrites,
          dependencies: &js.dependencies,
        };
        let rewritten = rewrite_specifiers(
          &parsed_source,
          &module_info,
          specifier_rewriter,
          RewriteKind::Source,
        );
        package_files
          .insert(js.specifier.path().to_owned(), rewritten.into_bytes());

        let source_target = source_rewrites.get(&js.specifier).unwrap();
        let specifier_rewriter = SpecifierRewriter {
          base_specifier: source_target,
          module_specifier: &js.specifier,
          source_rewrites: &source_rewrites,
          declaration_rewrites: &declaration_rewrites,
          dependencies: &js.dependencies,
        };
        let (source, source_map) = transpile_to_js(
          &parsed_source,
          specifier_rewriter,
          source_target,
          transpile_options,
          source_maps,
        )?;
        package_files.insert(source_target.path().to_owned(), source);
        if let Some(source_map) = source_map {
          package_files
            .insert(format!("{}.map", source_target.path()), source_map);
        }

        if let Some(fast_check_module) = js.fast_check_module() {
          let declaration_target =
            declaration_rewrites.get(&js.specifier).unwrap();
          let specifier_rewriter = SpecifierRewriter {
            base_specifier: declaration_target,
            module_specifier: &js.specifier,
            source_rewrites: &source_rewrites,
            declaration_rewrites: &declaration_rewrites,
            dependencies: &js.dependencies,
          };
          let (declaration, declaration_map) = transpile_to_dts(
            &parsed_source,
            fast_check_module,
            specifier_rewriter,
            declaration_target,
            source_maps,
          )?;
          package_files
            .insert(declaration_target.path().to_owned(), declaration);
          if let Some(declaration_map) = declaration_map {
            package_files.insert(
              format!("{}.map", declaration_target.path()),
              declaration_map,
            );
          }
        }
      }
      _ => {}
    }
  }

  let paths_to_read = files
    .paths()
    .filter(|path| !package_files.contains_key(*path))
    .collect::<Vec<_>>();
  let read_files = futures::stream::iter(paths_to_read)
    .map(|path| async move {
      Ok::<_, anyhow::Error>((path, files.read(path).await?))
    })
    .buffer_unordered(64)
    .try_collect::<Vec<_>>()
    .await?;
  for (path, content) in read_files {
    package_files.insert(path.to_string(), content);
  }

  let npm_exports = create_npm_exports(
    exports,
    &package_files,
    &source_rewrites,
    &declaration_rewrites,
  );

  let pkg_json = NpmPackageJson {
    name: package_json.name,
    version: package_json.version.to_string(),
    homepage: package_json.homepage,
    license: package_json.license,
    module_type: "module",
    exports: npm_exports,
    dependencies: npm_dependencies,
    revision: package_json.jsr_revision,
  };

  let pkg_json_str = serde_json::to_string_pretty(&pkg_json)?;
  package_files.insert("/package.json".to_string(), pkg_json_str.into());

  package_files.sort_keys();

  let mut tar_gz_bytes = Vec::new();
  let mut gz_encoder =
    GzEncoder::new(&mut tar_gz_bytes, Compression::default());
  let mut tarball = tar::Builder::new(&mut gz_encoder);
  for (path, content) in package_files.iter() {
    append_reproducible(&mut tarball, &format!("package{path}"), content)?;
  }
  tarball.into_inner()?;
  gz_encoder.finish()?;

  let entries = package_files
    .iter()
    .map(|(path, content)| NpmTarballEntry {
      path: path.clone(),
      size: content.len(),
    })
    .collect();

  // Free the in-memory file map now that the tarball is built
  drop(package_files);

  let sha1_digest = sha1::Sha1::digest(&tar_gz_bytes);
  let sha1 = format!("{sha1_digest:X}");
  let sha512_digest = sha2::Sha512::digest(&tar_gz_bytes);
  let sha512 = base64::prelude::BASE64_STANDARD.encode(sha512_digest);

  Ok(NpmTarball {
    tarball: tar_gz_bytes,
    sha1,
    sha512,
    entries,
  })
}

fn parse(
  parser: &dyn EsParser,
  js: &JsModule,
) -> Result<ParsedSource, anyhow::Error> {
  Ok(parser.parse_program(ParseOptions {
    specifier: &js.specifier,
    source: js.source.text.clone(),
    media_type: js.media_type,
    scope_analysis: false,
  })?)
}

/// Rejects archive paths that could escape the extraction root.
fn validate_tar_path(path: &str) -> std::io::Result<()> {
  let invalid = |reason: &str| {
    std::io::Error::new(
      std::io::ErrorKind::InvalidInput,
      format!("refusing to add {path} to tarball: {reason}"),
    )
  };
  for segment in path.split('/') {
    match segment {
      ".." => return Err(invalid("path contains a '..' component")),
      "." => return Err(invalid("path contains a '.' component")),
      "" => return Err(invalid("path is absolute or empty")),
      _ => {}
    }
  }
  Ok(())
}

/// Appends a file with a fixed mode, mtime, uid and gid, so the same package
/// always produces the same tarball.
fn append_reproducible(
  tar: &mut tar::Builder<impl std::io::Write>,
  path: &str,
  bytes: &[u8],
) -> std::io::Result<()> {
  validate_tar_path(path)?;
  let mut header = tar::Header::new_gnu();
  header.set_size(bytes.len() as u64);
  header.set_mode(0o644);
  header.set_mtime(0);
  header.set_uid(0);
  header.set_gid(0);
  // `append_data` writes a GNU long name entry for paths that don't fit the
  // 100 byte name field.
  tar.append_data(&mut header, path, bytes)
}

fn rewrite_specifiers(
  parsed_source: &ParsedSource,
  module_info: &ModuleInfo,
  specifier_rewriter: SpecifierRewriter,
  kind: RewriteKind,
) -> String {
  let source_text_info: &SourceTextInfo = parsed_source.text_info_lazy();
  let mut text_changes = vec![];

  let file_start_pos = source_text_info.range().start;

  let add_text_change = |text_changes: &mut Vec<TextChange>,
                         new_specifier: String,
                         range: &PositionRange| {
    let mut start = range
      .start
      .as_source_pos(source_text_info)
      .as_byte_index(file_start_pos);
    let mut end = range
      .end
      .as_source_pos(source_text_info)
      .as_byte_index(file_start_pos);

    let to_be_replaced = &source_text_info.text_str()[start..end];
    if to_be_replaced.starts_with('\'')
      || to_be_replaced.starts_with('"')
      || to_be_replaced.starts_with('`')
    {
      start += 1;
      end -= 1;
    }

    text_changes.push(TextChange {
      new_text: new_specifier,
      range: start..end,
    });
  };

  for desc in &module_info.dependencies {
    match desc {
      DependencyDescriptor::Static(desc) => {
        if let Some(specifier) =
          specifier_rewriter.rewrite(&desc.specifier, kind)
        {
          add_text_change(&mut text_changes, specifier, &desc.specifier_range);
        }
      }
      DependencyDescriptor::Dynamic(desc) => match &desc.argument {
        deno_graph::analysis::DynamicArgument::String(specifier) => {
          if let Some(specifier) = specifier_rewriter.rewrite(specifier, kind) {
            add_text_change(&mut text_changes, specifier, &desc.argument_range);
          }
        }
        deno_graph::analysis::DynamicArgument::Template(_) => {}
        deno_graph::analysis::DynamicArgument::Expr => {}
      },
    }
  }

  for ts_ref in &module_info.ts_references {
    match ts_ref {
      deno_graph::analysis::TypeScriptReference::Path(s) => {
        if let Some(specifier) =
          specifier_rewriter.rewrite(&s.text, RewriteKind::Declaration)
        {
          add_text_change(&mut text_changes, specifier, &s.range);
        }
      }
      deno_graph::analysis::TypeScriptReference::Types {
        specifier, ..
      } => {
        match kind {
          RewriteKind::Source => {
            // Type reference comments in JS are a Deno specific concept, and
            // are thus not relevant for the tarball. We remove them.

            let start_pos = source_text_info.range().start;
            let start = specifier.range.start.as_source_pos(source_text_info);
            let start = source_text_info.line_and_column_index(start);

            let line_start = source_text_info.line_start(start.line_index);
            let line_end = source_text_info.line_end(start.line_index);
            let line_text = source_text_info.line_text(start.line_index);

            let before = line_text[..start.column_index].to_string();

            let index = before.rfind("///").expect("should have ///");
            let comment_start = line_start + index;
            let comment_end = line_end;

            let range = comment_start.as_byte_index(start_pos)
              ..comment_end.as_byte_index(start_pos);

            text_changes.push(TextChange {
              new_text: "".to_string(),
              range,
            });
          }
          RewriteKind::Declaration => {
            if let Some(rewritten_specifier) = specifier_rewriter
              .rewrite(&specifier.text, RewriteKind::Declaration)
            {
              add_text_change(
                &mut text_changes,
                rewritten_specifier,
                &specifier.range,
              );
            }
          }
        }
      }
    }
  }

  for s in &module_info.jsdoc_imports {
    if let Some(specifier) =
      specifier_rewriter.rewrite(&s.specifier.text, RewriteKind::Declaration)
    {
      add_text_change(&mut text_changes, specifier, &s.specifier.range);
    }
  }

  if matches!(kind, RewriteKind::Source) {
    let mut collector = ImportMetaResolveCollector::default();
    parsed_source.program_ref().visit_with(&mut collector);
    for (range, specifier) in collector.specifiers {
      if let Some(new_text) = specifier_rewriter.rewrite_unanalyzed(&specifier)
      {
        // the range includes the quotes
        let range = range.as_byte_range(file_start_pos);
        text_changes.push(TextChange {
          new_text,
          range: range.start + 1..range.end - 1,
        });
      }
    }
  }

  apply_text_changes(source_text_info.text_str(), text_changes)
}

pub fn create_npm_dependencies<'a>(
  dependencies: impl Iterator<Item = Cow<'a, (DependencyKind, PackageReqReference)>>,
) -> Result<IndexMap<String, String>, anyhow::Error> {
  let mut npm_dependencies = IndexMap::new();
  for dep in dependencies {
    let (kind, req) = &*dep;
    match kind {
      DependencyKind::Jsr => {
        let npm_name =
          jsr_to_npm_package_name(&req.req.name).ok_or_else(|| {
            anyhow::anyhow!("invalid jsr package name: {}", req.req.name)
          })?;
        npm_dependencies.insert(npm_name, req.req.version_req.to_string());
      }
      DependencyKind::Npm => {
        npm_dependencies
          .insert(req.req.name.to_string(), req.req.version_req.to_string());
      }
    }
  }
  npm_dependencies.sort_keys();
  Ok(npm_dependencies)
}

fn create_npm_exports<'a>(
  exports: impl Iterator<Item = (&'a str, &'a str)>,
  package_files: &IndexMap<String, Vec<u8>>,
  source_rewrites: &HashMap<&ModuleSpecifier, ModuleSpecifier>,
  declaration_rewrites: &HashMap<&ModuleSpecifier, ModuleSpecifier>,
) -> IndexMap<String, NpmExportConditions> {
  let package_json_specifier =
    ModuleSpecifier::parse("file:///package.json").unwrap();

  let mut npm_exports = IndexMap::new();
  for (key, path) in exports {
    let mut conditions = NpmExportConditions {
      types: None,
      default: None,
    };

    let specifier = ModuleSpecifier::parse(&format!(
      "file:///{}",
      path.trim_start_matches('.').trim_start_matches('/')
    ))
    .unwrap();

    if let Some(source_specifier) =
      follow_specifier(&specifier, source_rewrites)
      && source_specifier.scheme() == "file"
      && package_files.contains_key(source_specifier.path())
    {
      let new_specifier =
        relative_import_specifier(&package_json_specifier, source_specifier);
      conditions.default = Some(new_specifier);
    }

    if let Some(types_specifier) =
      follow_specifier(&specifier, declaration_rewrites)
      && types_specifier.scheme() == "file"
      && package_files.contains_key(types_specifier.path())
    {
      let new_specifier =
        relative_import_specifier(&package_json_specifier, types_specifier);
      if conditions.default.as_ref() != Some(&new_specifier) {
        conditions.types = Some(new_specifier);
      }
    }

    npm_exports.insert(key.to_string(), conditions);
  }
  npm_exports
}
