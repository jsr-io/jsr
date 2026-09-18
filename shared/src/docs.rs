// Copyright 2024 the JSR authors. All rights reserved. MIT license.
use crate::ids::PackageName;
use crate::ids::ScopeName;
use crate::ids::Version;
use crate::models::ReadmeSource;
use crate::models::RuntimeCompat;
use anyhow::Context;
use bytes::Bytes;
use deno_ast::ModuleSpecifier;
use deno_doc::DeclarationDef;
use deno_doc::Location;
use deno_doc::ParseOutput;
use deno_doc::Symbol;
use deno_doc::html::DocNodeWithContext;
use deno_doc::html::GenerateCtx;
use deno_doc::html::HrefResolver;
use deno_doc::html::RenderContext;
use deno_doc::html::ShortPath;
use deno_doc::html::UrlResolveKind;
use deno_doc::html::UsageComposerEntry;
use deno_doc::html::pages::SymbolPage;
use deno_doc::html::util::BreadcrumbsCtx;
use deno_semver::RangeSetOrTag;
use flate2::Compression;
use flate2::write::GzEncoder;
use indexmap::IndexMap;
use serde::Deserialize;
use serde::Serialize;
use std::borrow::Cow;
use std::io::Write;
use std::sync::Arc;
use tracing::instrument;
use url::Url;

/// Current doc nodes storage format version.
pub const DOC_NODES_VERSION: u32 = 2;

/// Versioned wrapper for stored doc nodes.
#[derive(Serialize, Deserialize)]
pub struct StoredDocNodes {
  pub version: u32,
  pub doc_nodes: ParseOutput,
}

#[derive(Debug, thiserror::Error)]
pub enum DocNodeCacheError {
  #[error(transparent)]
  S3(#[from] crate::s3::S3Error),
  #[error("failed to decompress doc nodes: {0}")]
  Decompress(std::io::Error),
  #[error("failed to deserialize doc nodes: {0}")]
  Deserialize(String),
  #[error("failed to parse module specifier in legacy doc nodes: {0}")]
  InvalidSpecifier(url::ParseError),
  #[error("unexpected doc nodes JSON shape")]
  UnexpectedJsonShape,
  #[error("unsupported doc nodes version: {0} (expected {DOC_NODES_VERSION})")]
  UnsupportedVersion(u32),
}

/// Serialize doc nodes to gzip-compressed MessagePack with a version field.
pub fn serialize_doc_nodes(doc_nodes: &ParseOutput) -> Bytes {
  let stored = StoredDocNodes {
    version: DOC_NODES_VERSION,
    doc_nodes: doc_nodes.clone(),
  };
  let msgpack = rmp_serde::to_vec_named(&stored).unwrap();
  let mut encoder = GzEncoder::new(Vec::new(), Compression::default());
  encoder.write_all(&msgpack).unwrap();
  encoder.finish().unwrap().into()
}

pub type URLRewriter =
  Arc<dyn (Fn(Option<&ShortPath>, &str) -> String) + Send + Sync>;

#[derive(serde::Deserialize)]
pub struct WebType {
  pub id: Vec<String>,
  pub docs: String,
}

#[instrument(name = "generate_docs", skip(source_files, graph, analyzer), err)]
pub fn generate_docs(
  mut source_files: Vec<ModuleSpecifier>,
  graph: &deno_graph::ModuleGraph,
  analyzer: &deno_graph::ast::CapturingModuleAnalyzer,
) -> Result<ParseOutput, anyhow::Error> {
  source_files.sort();

  let parser = deno_doc::DocParser::new(
    graph,
    analyzer,
    &source_files,
    deno_doc::DocParserOptions {
      diagnostics: false,
      private: false,
    },
  )?;

  let doc_nodes_by_url = parser.parse()?;

  Ok(doc_nodes_by_url)
}

#[derive(Debug)]
pub enum DocsRequest {
  AllSymbols,
  Index,
  File(ModuleSpecifier),
  Symbol(ModuleSpecifier, String),
}

#[allow(clippy::large_enum_variant)]
#[derive(Debug)]
pub enum GeneratedDocsOutput {
  Docs(GeneratedDocs),
  Redirect(String),
}

#[derive(Debug)]
pub struct GeneratedDocs {
  pub breadcrumbs: Option<BreadcrumbsCtx>,
  pub toc: deno_doc::html::ToCCtx,
  pub main: GeneratedDocsContent,
}

#[derive(Debug)]
pub enum GeneratedDocsContent {
  AllSymbols(deno_doc::html::AllSymbolsCtx),
  File(deno_doc::html::jsdoc::ModuleDocCtx),
  Index(deno_doc::html::jsdoc::ModuleDocCtx),
  Symbol(deno_doc::html::SymbolGroupCtx),
}

pub struct DocsInfo {
  pub main_entrypoint: Option<Url>,
  pub entrypoint_url: Option<Url>,
  pub rewrite_map: IndexMap<Url, String>,
}

/// Whether `deno create @scope/package` works for this package: the CLI
/// rewrites that into `jsr:@scope/package/create`, so it needs a `./create`
/// export to run (jsr-io/jsr#1358).
pub fn has_create_export(exports: &crate::models::ExportsMap) -> bool {
  exports.contains_key("./create")
}

pub fn get_docs_info(
  exports: &crate::models::ExportsMap,
  entrypoint: Option<&str>,
) -> DocsInfo {
  let mut main_entrypoint = None;
  let mut entrypoint_url = None;
  let mut rewrite_map = IndexMap::new();

  let base_url = Url::parse("file:///").unwrap();

  for (name, path) in exports.iter() {
    let specifier = Url::options()
      .base_url(Some(&base_url))
      .parse(path)
      .unwrap();
    let key = if name == "." {
      main_entrypoint = Some(specifier.clone());

      name.as_str()
    } else {
      name.strip_prefix('.').unwrap_or(name)
    };
    if let Some(entrypoint) = entrypoint
      && key.strip_prefix('/').unwrap_or(key) == entrypoint
    {
      entrypoint_url = Some(specifier.clone());
    }
    rewrite_map.insert(specifier, key.into());
  }

  DocsInfo {
    main_entrypoint,
    entrypoint_url,
    rewrite_map,
  }
}

/// Maximum number of symbol rows rendered in module symbol listings (the
/// per-entrypoint overview and the "all symbols" page). Namespace-heavy
/// packages (e.g. zod, which re-exports its whole API under several
/// namespaces) can otherwise produce listings with tens of thousands of rows,
/// blowing the docs response past Cloud Run's 32 MiB response cap.
pub const SYMBOL_LISTING_LIMIT: usize = 2048;

#[allow(clippy::too_many_arguments)]
#[instrument(
  name = "get_generate_ctx",
  skip(
    documents_by_url,
    main_entrypoint,
    rewrite_map,
    scope,
    package,
    version,
    version_is_latest,
    runtime_compat,
    has_create_export,
    registry_url,
    diff,
    deno_types,
    markdown_renderer,
  )
)]
pub fn get_generate_ctx(
  doc_base: String,
  documents_by_url: ParseOutput,
  main_entrypoint: Option<ModuleSpecifier>,
  rewrite_map: IndexMap<ModuleSpecifier, String>,
  scope: ScopeName,
  package: PackageName,
  version: Version,
  version_is_latest: bool,
  runtime_compat: RuntimeCompat,
  has_create_export: bool,
  registry_url: String,
  diff: Option<(deno_doc::diff::DocDiff, bool)>,
  deno_types: Arc<std::collections::HashSet<Vec<String>>>,
  web_types: Arc<std::collections::HashMap<Vec<String>, String>>,
  markdown_renderer: deno_doc::html::jsdoc::MarkdownRenderer,
) -> GenerateCtx {
  let package_name = format!("@{scope}/{package}");

  GenerateCtx::new(
    deno_doc::html::GenerateOptions {
      package_name: Some(package_name),
      main_entrypoint,
      href_resolver: Arc::new(DocResolver {
        scope: scope.clone(),
        package: package.clone(),
        version: version.clone(),
        version_is_latest,
        registry_url,
        deno_types,
        web_types,
        doc_base,
        full: diff.as_ref().map(|diff| diff.1),
      }),
      usage_composer: diff.is_none().then(|| {
        Arc::new(DocUsageComposer {
          runtime_compat,
          scope,
          package,
          version,
          version_is_latest,
          has_create_export,
        }) as Arc<dyn deno_doc::html::UsageComposer>
      }),
      rewrite_map: Some(rewrite_map),
      category_docs: None,
      disable_search: false,
      symbol_redirect_map: None,
      default_symbol_map: None,
      markdown_renderer,
      markdown_stripper: Arc::new(deno_doc::html::comrak::strip),
      head_inject: None,
      id_prefix: None,
      diff_only: diff.as_ref().map(|diff| !diff.1).unwrap_or_default(),
      symbol_listing_limit: Some(SYMBOL_LISTING_LIMIT),
    },
    None,
    deno_doc::html::FileMode::Normal,
    documents_by_url,
    diff.map(|diff| diff.0),
  )
  .unwrap()
}

struct DocResolver {
  scope: ScopeName,
  package: PackageName,
  version: Version,
  version_is_latest: bool,
  registry_url: String,
  deno_types: Arc<std::collections::HashSet<Vec<String>>>,
  web_types: Arc<std::collections::HashMap<Vec<String>, String>>,
  doc_base: String,
  full: Option<bool>,
}

impl HrefResolver for DocResolver {
  fn resolve_path(
    &self,
    _current: UrlResolveKind,
    target: UrlResolveKind,
  ) -> String {
    let package_base = format!(
      "/@{}/{}{}",
      self.scope,
      self.package,
      if !self.version_is_latest {
        format!("@{}", self.version)
      } else {
        String::new()
      }
    );
    let doc_base = format!("{package_base}{}", self.doc_base);

    let path = match target {
      UrlResolveKind::Root => package_base,
      UrlResolveKind::AllSymbols => format!("{doc_base}/all_symbols"),
      UrlResolveKind::Symbol { file, symbol } => {
        format!(
          "{doc_base}{}/~/{symbol}",
          if file.is_main {
            String::new()
          } else {
            format!("/{}", file.path)
          }
        )
      }
      UrlResolveKind::File { file } => format!(
        "{doc_base}{}/",
        if file.is_main {
          String::new()
        } else {
          format!("/{}", file.path)
        }
      ),
      UrlResolveKind::Category { .. } => unreachable!(),
    };

    if self.full.is_some_and(std::convert::identity) {
      path + "?full"
    } else {
      path
    }
  }

  fn resolve_global_symbol(&self, symbol: &[String]) -> Option<String> {
    if let Some(mdn_docs) = self.web_types.get(symbol) {
      Some(mdn_docs.to_owned())
    } else if self.deno_types.contains(symbol) {
      Some(format!(
        "https://deno.land/api?unstable&s={}",
        symbol.join(".")
      ))
    } else {
      None
    }
  }

  fn resolve_import_href(
    &self,
    symbol: &[String],
    src: &str,
  ) -> Option<String> {
    if let Ok(url) = Url::parse(src) {
      match url.scheme() {
        "node" => Some(format!("https://nodejs.org/api/{}.html", url.path())),
        "bun" | "virtual" | "cloudflare" => None,
        "npm" => {
          let npm_package_req =
            deno_semver::npm::NpmPackageReqReference::from_str(src).ok()?;
          let req = npm_package_req.req();
          Some(format!(
            "https://www.npmjs.com/package/{}{}",
            req.name,
            match req.version_req.inner() {
              RangeSetOrTag::RangeSet(_) => String::new(),
              RangeSetOrTag::Tag(tag) => format!("/v/{tag}"),
            },
          ))
        }
        "http" | "https" if src.starts_with(&self.registry_url) => {
          let path_parts = url.path().splitn(4, '/').collect::<Vec<_>>();

          Some(format!(
            "/{}/{}@{}",
            path_parts[1], path_parts[2], path_parts[3]
          ))
        }
        "jsr" => {
          let symbol = symbol.join(".");
          let jsr_package_req =
            deno_semver::jsr::JsrPackageReqReference::from_str(src).ok()?;
          let req = jsr_package_req.req();

          let mut version_path = Cow::Borrowed("");
          if let Some(range) = req.version_req.range()
            && let Ok(version) = Version::new(&range.to_string())
          {
            // If using a specific version, link to it (e.g. prerelease)
            version_path = Cow::Owned(format!("@{}", version));
          }

          let mut internal_path = Cow::Borrowed("");
          if let Some(path) = jsr_package_req.sub_path() {
            internal_path = Cow::Owned(format!("/{path}"));
          }

          Some(format!(
            "/{}{version_path}/doc{internal_path}/~/{symbol}",
            req.name
          ))
        }
        _ => None,
      }
    } else {
      None
    }
  }

  fn resolve_source(&self, location: &Location) -> Option<String> {
    let url = Url::parse(&location.filename).ok()?;
    Some(format!(
      "/@{}/{}/{}{}#L{}",
      self.scope,
      self.package,
      self.version,
      url.path(),
      location.line,
    ))
  }

  fn resolve_external_jsdoc_module(
    &self,
    _module: &str,
    _symbol: Option<&str>,
  ) -> Option<(String, String)> {
    None
  }
}

struct DocUsageComposer {
  runtime_compat: RuntimeCompat,
  scope: ScopeName,
  package: PackageName,
  version: Version,
  version_is_latest: bool,
  /// Whether the package has a `./create` export. `deno create @scope/pkg`
  /// runs `jsr:@scope/pkg/create`, so that export is exactly what makes the
  /// command work (jsr-io/jsr#1358).
  has_create_export: bool,
}

impl deno_doc::html::UsageComposer for DocUsageComposer {
  fn is_single_mode(&self) -> bool {
    false
  }

  #[allow(clippy::nonminimal_bool)]
  fn compose(
    &self,
    current_resolve: UrlResolveKind,
    usage_to_md: deno_doc::html::UsageToMd,
  ) -> IndexMap<UsageComposerEntry, String> {
    let mut map = IndexMap::new();
    // When the user is looking at a pinned, non-latest version, the add
    // commands install exactly that version instead of the latest one.
    let scoped_name = if self.version_is_latest {
      format!("@{}/{}", self.scope, self.package)
    } else {
      format!("@{}/{}@{}", self.scope, self.package, self.version)
    };

    let (is_main, path) = current_resolve
      .get_file()
      .map(|short_path| (short_path.is_main, &*short_path.path))
      .unwrap_or((true, ""));

    let subpath = if is_main {
      String::new()
    } else {
      format!("/{path}")
    };

    let url = format!("@{}/{}{}", self.scope, self.package, subpath);

    // A direct jsr specifier always carries a version constraint so the
    // snippet passes Deno's no-unversioned-import lint rule: a caret range on
    // the latest version (matching what `deno add` writes), and the exact
    // version when a non-latest version is pinned.
    let jsr_url = format!(
      "@{}/{}@{}{}{}",
      self.scope,
      self.package,
      if self.version_is_latest { "^" } else { "" },
      self.version,
      subpath
    );

    let import = format!(
      "\nImport symbol\n{}",
      usage_to_md(&url, Some(self.package.as_str()))
    );

    if !self.runtime_compat.deno.is_some_and(|compat| !compat) {
      // Packages exporting `./create` are project scaffolds, so lead with the
      // command that actually runs them.
      let scaffold = if self.has_create_export {
        format!("Scaffold a project\n```\ndeno create {scoped_name}\n```\n")
      } else {
        String::new()
      };

      map.insert(
        UsageComposerEntry {
          name: "Deno".to_string(),
          icon: Some("/logos/deno.svg".into()),
        },
        format!("{scaffold}Add Package\n```\ndeno add jsr:{scoped_name}\n```{import}\n<div class='or-bar'>or</div>\n\nImport directly with a jsr specifier\n{}\n", usage_to_md(&format!("jsr:{jsr_url}"), Some(self.package.as_str()))),
      );
    }

    // Packages that only target browsers or Cloudflare Workers are still
    // installed through npm-ecosystem package managers (via a bundler), so an
    // explicit browser/workerd "yes" shows these tabs even when node is "no".
    if !self.runtime_compat.node.is_some_and(|compat| !compat)
      || self.runtime_compat.browser.is_some_and(|compat| compat)
      || self.runtime_compat.workerd.is_some_and(|compat| compat)
    {
      map.insert(
        UsageComposerEntry {
          name: "pnpm".to_string(),
          icon: Some("/logos/pnpm_textless.svg".into()),
        },
        format!("Add Package\n```\npnpm i jsr:{scoped_name}\n```\n<div class='or-bar'>or (using pnpm 10.8 or older)</div>\n\n```\npnpm dlx jsr add {scoped_name}\n```{import}"),
      );
      map.insert(
        UsageComposerEntry {
          name: "Yarn".to_string(),
          icon: Some("/logos/yarn_textless.svg".into()),
        },
        format!("Add Package\n```\nyarn add jsr:{scoped_name}\n```\n<div class='or-bar'>or (using Yarn 4.8 or older)</div>\n\n```\nyarn dlx jsr add {scoped_name}\n```{import}"),
      );
      map.insert(
        UsageComposerEntry {
          name: "vlt".to_string(),
          icon: Some("/logos/vlt.svg".into()),
        },
        format!("Add Package\n```\nvlt install jsr:{scoped_name}\n```{import}"),
      );
      map.insert(
        UsageComposerEntry {
          name: "npm".to_string(),
          icon: Some("/logos/npm_textless.svg".into()),
        },
        format!("Add Package\n```\nnpx jsr add {scoped_name}\n```{import}"),
      );
    }

    if !self.runtime_compat.bun.is_some_and(|compat| !compat) {
      map.insert(
        UsageComposerEntry {
          name: "Bun".to_string(),
          icon: Some("/logos/bun.svg".into()),
        },
        format!("Add Package\n```\nbunx jsr add {scoped_name}\n```{import}"),
      );
    }

    map
  }
}

#[cfg(test)]
mod tests {
  use super::*;
  use deno_doc::html::ShortPath;

  #[test]
  fn url_resolver_test() {
    let resolver = DocResolver {
      scope: ScopeName::new("foo".to_string()).unwrap(),
      package: PackageName::new("bar".to_string()).unwrap(),
      version: Version::new("0.0.1").unwrap(),
      version_is_latest: false,
      registry_url: "".to_string(),
      deno_types: Default::default(),
      web_types: Default::default(),
      doc_base: "/doc".to_string(),
      full: None,
    };

    let specifier = ModuleSpecifier::parse("file:///mod.ts").unwrap();
    let short_path = ShortPath::new(
      specifier.clone(),
      None,
      Some(&IndexMap::from([(specifier, "mod".to_string())])),
      None,
    );

    {
      assert_eq!(
        resolver.resolve_path(UrlResolveKind::Root, UrlResolveKind::Root),
        "/@foo/bar@0.0.1"
      );
      assert_eq!(
        resolver.resolve_path(UrlResolveKind::Root, UrlResolveKind::AllSymbols),
        "/@foo/bar@0.0.1/doc/all_symbols"
      );
      assert_eq!(
        resolver.resolve_path(
          UrlResolveKind::Root,
          UrlResolveKind::File { file: &short_path }
        ),
        "/@foo/bar@0.0.1/doc/mod/"
      );
      assert_eq!(
        resolver.resolve_path(
          UrlResolveKind::Root,
          UrlResolveKind::Symbol {
            file: &short_path,
            symbol: "bar",
          }
        ),
        "/@foo/bar@0.0.1/doc/mod/~/bar"
      );
    }

    {
      assert_eq!(
        resolver.resolve_path(UrlResolveKind::AllSymbols, UrlResolveKind::Root),
        "/@foo/bar@0.0.1"
      );
      assert_eq!(
        resolver
          .resolve_path(UrlResolveKind::AllSymbols, UrlResolveKind::AllSymbols),
        "/@foo/bar@0.0.1/doc/all_symbols"
      );
      assert_eq!(
        resolver.resolve_path(
          UrlResolveKind::AllSymbols,
          UrlResolveKind::File { file: &short_path }
        ),
        "/@foo/bar@0.0.1/doc/mod/"
      );
      assert_eq!(
        resolver.resolve_path(
          UrlResolveKind::AllSymbols,
          UrlResolveKind::Symbol {
            file: &short_path,
            symbol: "bar",
          }
        ),
        "/@foo/bar@0.0.1/doc/mod/~/bar"
      );
    }

    {
      assert_eq!(
        resolver.resolve_path(
          UrlResolveKind::File { file: &short_path },
          UrlResolveKind::Root
        ),
        "/@foo/bar@0.0.1"
      );
      assert_eq!(
        resolver.resolve_path(
          UrlResolveKind::File { file: &short_path },
          UrlResolveKind::AllSymbols
        ),
        "/@foo/bar@0.0.1/doc/all_symbols"
      );
      assert_eq!(
        resolver.resolve_path(
          UrlResolveKind::File { file: &short_path },
          UrlResolveKind::File { file: &short_path }
        ),
        "/@foo/bar@0.0.1/doc/mod/"
      );
      assert_eq!(
        resolver.resolve_path(
          UrlResolveKind::File { file: &short_path },
          UrlResolveKind::Symbol {
            file: &short_path,
            symbol: "bar",
          }
        ),
        "/@foo/bar@0.0.1/doc/mod/~/bar"
      );
    }

    {
      assert_eq!(
        resolver.resolve_path(
          UrlResolveKind::Symbol {
            file: &short_path,
            symbol: "bar"
          },
          UrlResolveKind::Root
        ),
        "/@foo/bar@0.0.1"
      );
      assert_eq!(
        resolver.resolve_path(
          UrlResolveKind::Symbol {
            file: &short_path,
            symbol: "bar"
          },
          UrlResolveKind::AllSymbols
        ),
        "/@foo/bar@0.0.1/doc/all_symbols"
      );
      assert_eq!(
        resolver.resolve_path(
          UrlResolveKind::Symbol {
            file: &short_path,
            symbol: "bar"
          },
          UrlResolveKind::File { file: &short_path }
        ),
        "/@foo/bar@0.0.1/doc/mod/"
      );
      assert_eq!(
        resolver.resolve_path(
          UrlResolveKind::Symbol {
            file: &short_path,
            symbol: "bar"
          },
          UrlResolveKind::Symbol {
            file: &short_path,
            symbol: "bar",
          }
        ),
        "/@foo/bar@0.0.1/doc/mod/~/bar"
      );
    }

    {
      assert_eq!(
        resolver
          .resolve_import_href(
            &["Expression".to_string()],
            "jsr:@babel/types@0.0.0-beta.1"
          )
          .as_deref(),
        Some("/@babel/types@0.0.0-beta.1/doc/~/Expression")
      );

      assert_eq!(
        resolver
          .resolve_import_href(
            &["version".to_string()],
            "jsr:@babel/core/package.json"
          )
          .as_deref(),
        Some("/@babel/core/doc/package.json/~/version")
      );
    }
  }

  fn compose_deno_usage(has_create_export: bool) -> String {
    use deno_doc::html::UsageComposer;

    let composer = DocUsageComposer {
      runtime_compat: RuntimeCompat {
        browser: None,
        deno: None,
        node: None,
        workerd: None,
        bun: None,
      },
      scope: ScopeName::new("foo".to_string()).unwrap(),
      package: PackageName::new("bar".to_string()).unwrap(),
      version: Version::new("1.0.0").unwrap(),
      version_is_latest: true,
      has_create_export,
    };

    let usage_to_md = |url: &str, _name: Option<&str>| format!("`{url}`");
    let usages = composer.compose(UrlResolveKind::Root, &usage_to_md);

    usages
      .into_iter()
      .find(|(entry, _)| entry.name == "Deno")
      .unwrap()
      .1
  }

  #[test]
  fn create_export_adds_scaffold_command() {
    let with_create = compose_deno_usage(true);
    assert!(
      with_create.contains("deno create @foo/bar"),
      "{with_create}"
    );
    // the scaffold block leads, but installing is still offered
    assert!(
      with_create.find("deno create @foo/bar").unwrap()
        < with_create.find("deno add jsr:@foo/bar").unwrap(),
      "{with_create}"
    );

    // a package without the export must not advertise a command that fails
    let without_create = compose_deno_usage(false);
    assert!(!without_create.contains("deno create"), "{without_create}");
    assert!(
      without_create.contains("deno add jsr:@foo/bar"),
      "{without_create}"
    );
  }

  #[test]
  fn has_create_export_checks_the_export_key() {
    let with_create = crate::models::ExportsMap::new(IndexMap::from([
      (".".to_string(), "./mod.ts".to_string()),
      ("./create".to_string(), "./create.ts".to_string()),
    ]));
    assert!(has_create_export(&with_create));

    // a `create` *file* under some other export must not count
    let without_create = crate::models::ExportsMap::new(IndexMap::from([
      (".".to_string(), "./mod.ts".to_string()),
      ("./cli".to_string(), "./create.ts".to_string()),
    ]));
    assert!(!has_create_export(&without_create));
  }
}
