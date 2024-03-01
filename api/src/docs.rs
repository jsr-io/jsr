// Copyright 2024 the JSR authors. All rights reserved. MIT license.
use crate::db::RuntimeCompat;
use crate::ids::PackageName;
use crate::ids::ScopeName;
use crate::ids::Version;
use anyhow::Context;
use deno_ast::ModuleSpecifier;
use deno_doc::html::DocNodeWithContext;
use deno_doc::html::HrefResolver;
use deno_doc::html::ShortPath;
use deno_doc::html::UrlResolveKind;
use deno_doc::DocNode;
use deno_doc::Location;
use deno_semver::RangeSetOrTag;
use indexmap::IndexMap;
use std::borrow::Cow;
use std::rc::Rc;
use std::sync::OnceLock;
use tracing::instrument;
use url::Url;

pub type DocNodesByUrl = IndexMap<ModuleSpecifier, Vec<DocNode>>;

static DENO_TYPES: OnceLock<std::collections::HashSet<Vec<String>>> =
  OnceLock::new();
static WEB_TYPES: OnceLock<std::collections::HashMap<Vec<String>, String>> =
  OnceLock::new();

#[derive(serde::Deserialize)]
struct WebType {
  id: Vec<String>,
  docs: String,
}

#[instrument(name = "generate_docs", skip(source_files, graph, analyzer), err)]
pub fn generate_docs(
  mut source_files: Vec<ModuleSpecifier>,
  graph: &deno_graph::ModuleGraph,
  analyzer: &deno_graph::CapturingModuleAnalyzer,
) -> Result<DocNodesByUrl, anyhow::Error> {
  let parser = deno_doc::DocParser::new(
    graph,
    analyzer,
    deno_doc::DocParserOptions {
      diagnostics: false,
      private: false,
    },
  )?;

  source_files.sort();
  let mut doc_nodes_by_url = IndexMap::with_capacity(source_files.len());
  for source_file in &source_files {
    let nodes = parser.parse_with_reexports(source_file)?;
    doc_nodes_by_url.insert(source_file.to_owned(), nodes);
  }

  Ok(doc_nodes_by_url)
}

#[derive(Debug)]
pub enum DocsRequest {
  AllSymbols,
  Index,
  File(ModuleSpecifier),
  Symbol(ModuleSpecifier, String),
}

pub struct GeneratedDocs {
  pub breadcrumbs: Option<String>,
  pub sidepanel: Option<String>,
  pub main: String,
}

pub struct DocsInfo {
  pub main_entrypoint: Option<Url>,
  pub entrypoint_url: Option<Url>,
  pub rewrite_map: IndexMap<Url, String>,
}

pub fn get_docs_info(
  version: &crate::db::PackageVersion,
  entrypoint: Option<&str>,
) -> DocsInfo {
  let mut main_entrypoint = None;
  let mut entrypoint_url = None;
  let mut rewrite_map = IndexMap::new();

  let base_url = Url::parse("file:///").unwrap();

  for (name, path) in version.exports.iter() {
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
    if let Some(entrypoint) = entrypoint {
      if key.strip_prefix('/').unwrap_or(key) == entrypoint {
        entrypoint_url = Some(specifier.clone());
      }
    }
    rewrite_map.insert(specifier, key.into());
  }

  DocsInfo {
    main_entrypoint,
    entrypoint_url,
    rewrite_map,
  }
}

fn get_url_rewriter(
  base: String,
  is_readme: bool,
) -> deno_doc::html::comrak_adapters::URLRewriter {
  std::sync::Arc::new(move |current_specifier, url| {
    if url.starts_with('#') || url.starts_with('/') {
      return url.to_string();
    }

    if !is_readme {
      if let Some(current_specifier) = current_specifier {
        let (path, _file) = current_specifier
          .path()
          .rsplit_once('/')
          .unwrap_or((current_specifier.path(), ""));
        return format!("{base}{path}/{url}");
      }
    }

    format!("{base}/{url}")
  })
}

#[allow(clippy::too_many_arguments)]
#[instrument(
  name = "get_generate_ctx",
  skip(
    doc_nodes_by_url,
    main_entrypoint,
    rewrite_map,
    scope,
    package,
    version,
    version_is_latest,
    has_readme,
    runtime_compat,
    registry_url
  )
)]
pub fn get_generate_ctx(
  doc_nodes_by_url: &DocNodesByUrl,
  main_entrypoint: Option<ModuleSpecifier>,
  rewrite_map: IndexMap<ModuleSpecifier, String>,
  scope: ScopeName,
  package: PackageName,
  version: Version,
  version_is_latest: bool,
  has_readme: bool,
  runtime_compat: RuntimeCompat,
  registry_url: String,
) -> deno_doc::html::GenerateCtx {
  let url_rewriter_base = format!("/@{scope}/{package}/{version}");

  deno_doc::html::GenerateCtx {
    package_name: None,
    common_ancestor: None,
    main_entrypoint,
    specifiers: doc_nodes_by_url.keys().cloned().collect(),
    hbs: deno_doc::html::setup_hbs().unwrap(),
    highlight_adapter: deno_doc::html::setup_highlighter(false),
    url_rewriter: Some(get_url_rewriter(url_rewriter_base, has_readme)),
    href_resolver: Rc::new(DocResolver {
      scope: scope.clone(),
      package: package.clone(),
      version,
      version_is_latest,
      registry_url,
      deno_types: DENO_TYPES
        .get_or_init(|| {
          serde_json::from_str(include_str!("./docs/deno_types.json")).unwrap()
        })
        .clone(),
      web_types: WEB_TYPES
        .get_or_init(|| {
          serde_json::from_str::<Vec<WebType>>(include_str!(
            "./docs/web_builtins.json"
          ))
          .unwrap()
          .into_iter()
          .map(|web_type| (web_type.id, web_type.docs))
          .collect()
        })
        .clone(),
    }),
    usage_composer: Some(Rc::new(move |ctx, doc_nodes, url| {
      let mut map = IndexMap::new();

      if !runtime_compat.deno.is_some_and(|compat| !compat) {
        let scoped_name = format!("@{scope}/{package}");
        let import =
          deno_doc::html::usage_to_md(ctx, doc_nodes, scoped_name.clone());
        map.insert(
          "Deno".to_string(),
          format!("```\ndeno add {scoped_name}\n```\n{import}"),
        );
      }

      if !runtime_compat.node.is_some_and(|compat| !compat) {
        let scoped_name = format!("@{scope}/{package}");
        let import =
          deno_doc::html::usage_to_md(ctx, doc_nodes, scoped_name.clone());
        map.insert(
          "npm".to_string(),
          format!("```\nnpx jsr add {scoped_name}\n```\n{import}"),
        );
        map.insert(
          "Yarn".to_string(),
          format!("```\nyarn dlx jsr add {scoped_name}\n```\n{import}"),
        );
        map.insert(
          "pnpm".to_string(),
          format!("```\npnpm dlx jsr add {scoped_name}\n```\n{import}"),
        );
      }

      if !runtime_compat.bun.is_some_and(|compat| !compat) {
        let scoped_name = format!("@{scope}/{package}");
        let import =
          deno_doc::html::usage_to_md(ctx, doc_nodes, scoped_name.clone());
        map.insert(
          "Bun".to_string(),
          format!("```\nbunx jsr add {scoped_name}\n```\n{import}"),
        );
      }

      map
    })),
    rewrite_map: Some(rewrite_map),
    hide_module_doc_title: true,
    single_file_mode: false,
    sidebar_hide_all_symbols: true,
    sidebar_flatten_namespaces: false,
  }
}

#[allow(clippy::too_many_arguments)]
#[instrument(
  name = "generate_docs_html",
  skip(doc_nodes_by_url, rewrite_map, readme),
  err
)]
pub fn generate_docs_html(
  doc_nodes_by_url: &DocNodesByUrl,
  main_entrypoint: Option<ModuleSpecifier>,
  rewrite_map: IndexMap<ModuleSpecifier, String>,
  req: DocsRequest,
  scope: ScopeName,
  package: PackageName,
  version: Version,
  version_is_latest: bool,
  readme: Option<String>,
  runtime_compat: RuntimeCompat,
  registry_url: String,
) -> Result<Option<GeneratedDocs>, anyhow::Error> {
  let ctx = get_generate_ctx(
    doc_nodes_by_url,
    main_entrypoint,
    rewrite_map,
    scope,
    package,
    version,
    version_is_latest,
    readme.is_some(),
    runtime_compat,
    registry_url,
  );

  match req {
    DocsRequest::AllSymbols => {
      let render_ctx = deno_doc::html::RenderContext::new(
        &ctx,
        &[],
        UrlResolveKind::AllSymbols,
        None,
      );

      let all_doc_nodes = doc_nodes_by_url
        .iter()
        .flat_map(|(specifier, nodes)| {
          nodes.iter().map(|node| DocNodeWithContext {
            origin: Some(Cow::Owned(ctx.url_to_short_path(specifier))),
            doc_node: node,
          })
        })
        .collect::<Vec<_>>();

      let partitions_by_kind =
        deno_doc::html::namespace::partition_nodes_by_kind(
          &all_doc_nodes,
          true,
        );

      let sections = deno_doc::html::namespace::render_namespace(
        &render_ctx,
        partitions_by_kind,
      );

      let breadcrumbs = ctx
        .hbs
        .render("breadcrumbs", &render_ctx.get_breadcrumbs())
        .context("failed to render breadcrumbs")?;
      let main = ctx
        .hbs
        .render(
          "symbol_content",
          &deno_doc::html::SymbolContentCtx {
            id: String::new(),
            sections,
            docs: None,
          },
        )
        .context("failed to all symbols list")?;

      Ok(Some(GeneratedDocs {
        breadcrumbs: Some(breadcrumbs),
        sidepanel: None,
        main,
      }))
    }
    DocsRequest::Index => {
      let doc_nodes = ctx
        .main_entrypoint
        .as_ref()
        .and_then(|specifier| doc_nodes_by_url.get(specifier).map(|v| &**v))
        .unwrap_or(&[]);

      let render_ctx = deno_doc::html::RenderContext::new(
        &ctx,
        doc_nodes,
        UrlResolveKind::Root,
        ctx.main_entrypoint.as_ref(),
      );

      let mut index_module_doc = ctx
        .main_entrypoint
        .as_ref()
        .map(|main_entrypoint| {
          deno_doc::html::jsdoc::ModuleDocCtx::new(
            &render_ctx,
            main_entrypoint,
            doc_nodes_by_url,
          )
        })
        .unwrap_or_default();
      if index_module_doc.docs.is_none() {
        index_module_doc.docs = Some(
          readme
            .as_ref()
            .map(|readme| {
              deno_doc::html::jsdoc::markdown_to_html(
                &render_ctx,
                readme,
                false,
                true,
              )
            })
            .unwrap_or(deno_doc::html::jsdoc::Markdown {
              html: r#"<div style="font-style: italic;">No docs found.</div>"#
                .to_string(),
              toc: None,
            }),
        );
      }

      let partitions_for_main_entrypoint =
        deno_doc::html::get_partitions_for_main_entrypoint(
          &ctx,
          doc_nodes_by_url,
        );
      let index_sidepanel = deno_doc::html::sidepanels::IndexSidepanelCtx::new(
        &ctx,
        ctx.main_entrypoint.as_ref(),
        doc_nodes_by_url,
        partitions_for_main_entrypoint,
        None,
      );
      let sidepanel = ctx
        .hbs
        .render("index_sidepanel", &index_sidepanel)
        .context("failed to render index sidepanel")?;

      let main = ctx
        .hbs
        .render("module_doc", &index_module_doc)
        .context("failed to render index module doc")?;

      Ok(Some(GeneratedDocs {
        breadcrumbs: None,
        sidepanel: Some(sidepanel),
        main,
      }))
    }
    DocsRequest::File(specifier) => {
      let doc_nodes = doc_nodes_by_url
        .get(&specifier)
        .map(|v| &**v)
        .context("doc nodes missing for specifier")?;

      let short_path = ctx.url_to_short_path(&specifier);
      let partitions_for_nodes = deno_doc::html::get_partitions_for_file(
        &ctx,
        doc_nodes,
        Cow::Borrowed(&short_path),
      );

      let render_ctx = deno_doc::html::RenderContext::new(
        &ctx,
        doc_nodes,
        UrlResolveKind::File(&short_path),
        Some(&specifier),
      );

      let module_doc = deno_doc::html::jsdoc::ModuleDocCtx::new(
        &render_ctx,
        &specifier,
        doc_nodes_by_url,
      );

      let breadcrumbs = ctx
        .hbs
        .render("breadcrumbs", &render_ctx.get_breadcrumbs())
        .context("failed to render breadcrumbs")?;

      let sidepanel = deno_doc::html::sidepanels::IndexSidepanelCtx::new(
        &ctx,
        Some(&specifier),
        doc_nodes_by_url,
        partitions_for_nodes,
        Some(&short_path),
      );
      let sidepanel = ctx
        .hbs
        .render("index_sidepanel", &sidepanel)
        .context("failed to render index sidepanel")?;

      let main = ctx
        .hbs
        .render("module_doc", &module_doc)
        .context("failed to render module doc")?;

      Ok(Some(GeneratedDocs {
        breadcrumbs: Some(breadcrumbs),
        sidepanel: Some(sidepanel),
        main,
      }))
    }
    DocsRequest::Symbol(specifier, symbol) => {
      let doc_nodes = doc_nodes_by_url
        .get(&specifier)
        .map(|v| &**v)
        .context("doc nodes missing for specifier")?;
      let short_path = ctx.url_to_short_path(&specifier);
      let partitions_for_nodes = deno_doc::html::get_partitions_for_file(
        &ctx,
        doc_nodes,
        Cow::Borrowed(&short_path),
      );

      let Some((breadcrumbs_ctx, sidepanel_ctx, symbol_group_ctx)) =
        deno_doc::html::generate_symbol_page(
          &ctx,
          &specifier,
          &short_path,
          &partitions_for_nodes,
          doc_nodes,
          &symbol,
        )
      else {
        return Ok(None);
      };

      let breadcrumbs = ctx
        .hbs
        .render("breadcrumbs", &breadcrumbs_ctx)
        .context("failed to render breadcrumbs")?;

      let sidepanel = ctx
        .hbs
        .render("sidepanel", &sidepanel_ctx)
        .context("failed to render sidepanel")?;

      let main = ctx
        .hbs
        .render("symbol_group", &symbol_group_ctx)
        .context("failed to render symbol group")?;

      Ok(Some(GeneratedDocs {
        breadcrumbs: Some(breadcrumbs),
        sidepanel: Some(sidepanel),
        main,
      }))
    }
  }
}

struct DocResolver {
  scope: ScopeName,
  package: PackageName,
  version: Version,
  version_is_latest: bool,
  registry_url: String,
  deno_types: std::collections::HashSet<Vec<String>>,
  web_types: std::collections::HashMap<Vec<String>, String>,
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
    let doc_base = format!("{package_base}/doc");

    match target {
      UrlResolveKind::Root => package_base,
      UrlResolveKind::AllSymbols => doc_base,
      UrlResolveKind::Symbol { file, symbol } => {
        format!(
          "{doc_base}{}/~/{symbol}",
          if file.as_str() == "." {
            ""
          } else {
            file.as_str()
          }
        )
      }
      UrlResolveKind::File(file) => format!(
        "{doc_base}{}/~/",
        if file.as_str() == "." {
          ""
        } else {
          file.as_str()
        }
      ),
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
          let symbol = symbol.join(".");
          let path_parts = url.path().splitn(4, '/').collect::<Vec<_>>();

          Some(format!(
            "/{}/{}@{}/doc/{}/~/{symbol}",
            path_parts[0], path_parts[1], path_parts[2], path_parts[3]
          ))
        }
        "jsr" => {
          let symbol = symbol.join(".");
          let jsr_package_req =
            deno_semver::jsr::JsrPackageReqReference::from_str(src).ok()?;
          let req = jsr_package_req.req();

          Some(format!("/{}/~/{symbol}", req.name))
        }
        _ => None,
      }
    } else {
      None
    }
  }

  fn resolve_usage(
    &self,
    _current_specifier: &ModuleSpecifier,
    current_file: Option<&ShortPath>,
  ) -> Option<String> {
    Some(format!(
      "jsr:@{}/{}@{}{}",
      self.scope,
      self.package,
      if self.version.0.major > 0 {
        self.version.0.major.to_string()
      } else {
        format!("0.{}", self.version.0.minor)
      },
      if let Some(current_file) = current_file {
        if current_file.as_str() == "." {
          ""
        } else {
          current_file.as_str()
        }
      } else {
        ""
      }
    ))
  }

  fn resolve_source(&self, location: &Location) -> Option<String> {
    let url =
      Url::parse(&location.filename).expect("filename was generated with Url");
    Some(format!(
      "/@{}/{}/{}{}#L{}",
      self.scope,
      self.package,
      self.version,
      url.path(),
      location.line,
    ))
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
    };

    {
      assert_eq!(
        resolver.resolve_path(UrlResolveKind::Root, UrlResolveKind::Root),
        "/@foo/bar@0.0.1"
      );
      assert_eq!(
        resolver.resolve_path(UrlResolveKind::Root, UrlResolveKind::AllSymbols),
        "/@foo/bar@0.0.1/doc"
      );
      assert_eq!(
        resolver.resolve_path(
          UrlResolveKind::Root,
          UrlResolveKind::File(&ShortPath::from("/mod.ts".to_string()))
        ),
        "/@foo/bar@0.0.1/doc/mod.ts/~/"
      );
      assert_eq!(
        resolver.resolve_path(
          UrlResolveKind::Root,
          UrlResolveKind::Symbol {
            file: &ShortPath::from("/mod.ts".to_string()),
            symbol: "bar",
          }
        ),
        "/@foo/bar@0.0.1/doc/mod.ts/~/bar"
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
        "/@foo/bar@0.0.1/doc"
      );
      assert_eq!(
        resolver.resolve_path(
          UrlResolveKind::AllSymbols,
          UrlResolveKind::File(&ShortPath::from("/mod.ts".to_string()))
        ),
        "/@foo/bar@0.0.1/doc/mod.ts/~/"
      );
      assert_eq!(
        resolver.resolve_path(
          UrlResolveKind::AllSymbols,
          UrlResolveKind::Symbol {
            file: &ShortPath::from("/mod.ts".to_string()),
            symbol: "bar",
          }
        ),
        "/@foo/bar@0.0.1/doc/mod.ts/~/bar"
      );
    }

    {
      assert_eq!(
        resolver.resolve_path(
          UrlResolveKind::File(&ShortPath::from("/mod.ts".to_string())),
          UrlResolveKind::Root
        ),
        "/@foo/bar@0.0.1"
      );
      assert_eq!(
        resolver.resolve_path(
          UrlResolveKind::File(&ShortPath::from("/mod.ts".to_string())),
          UrlResolveKind::AllSymbols
        ),
        "/@foo/bar@0.0.1/doc"
      );
      assert_eq!(
        resolver.resolve_path(
          UrlResolveKind::File(&ShortPath::from("/mod.ts".to_string())),
          UrlResolveKind::File(&ShortPath::from("/mod.ts".to_string()))
        ),
        "/@foo/bar@0.0.1/doc/mod.ts/~/"
      );
      assert_eq!(
        resolver.resolve_path(
          UrlResolveKind::File(&ShortPath::from("/mod.ts".to_string())),
          UrlResolveKind::Symbol {
            file: &ShortPath::from("/mod.ts".to_string()),
            symbol: "bar",
          }
        ),
        "/@foo/bar@0.0.1/doc/mod.ts/~/bar"
      );
    }

    {
      assert_eq!(
        resolver.resolve_path(
          UrlResolveKind::Symbol {
            file: &ShortPath::from("/mod.ts".to_string()),
            symbol: "bar"
          },
          UrlResolveKind::Root
        ),
        "/@foo/bar@0.0.1"
      );
      assert_eq!(
        resolver.resolve_path(
          UrlResolveKind::Symbol {
            file: &ShortPath::from("/mod.ts".to_string()),
            symbol: "bar"
          },
          UrlResolveKind::AllSymbols
        ),
        "/@foo/bar@0.0.1/doc"
      );
      assert_eq!(
        resolver.resolve_path(
          UrlResolveKind::Symbol {
            file: &ShortPath::from("/mod.ts".to_string()),
            symbol: "bar"
          },
          UrlResolveKind::File(&ShortPath::from("/mod.ts".to_string()))
        ),
        "/@foo/bar@0.0.1/doc/mod.ts/~/"
      );
      assert_eq!(
        resolver.resolve_path(
          UrlResolveKind::Symbol {
            file: &ShortPath::from("/mod.ts".to_string()),
            symbol: "bar"
          },
          UrlResolveKind::Symbol {
            file: &ShortPath::from("/mod.ts".to_string()),
            symbol: "bar",
          }
        ),
        "/@foo/bar@0.0.1/doc/mod.ts/~/bar"
      );
    }
  }

  #[test]
  fn test_url_rewriter() {
    let base = String::from("/@foo/bar/1.2.3");
    let rewriter = get_url_rewriter(base.clone(), false);

    assert_eq!(rewriter(None, "#hello"), "#hello");

    assert_eq!(
      rewriter(None, "src/assets/logo.svg"),
      "/@foo/bar/1.2.3/src/assets/logo.svg"
    );

    assert_eq!(
      rewriter(
        Some(&Url::parse("file:///src/mod.ts").unwrap()),
        "./logo.svg"
      ),
      "/@foo/bar/1.2.3/src/./logo.svg"
    );

    let rewriter = get_url_rewriter(base, true);

    assert_eq!(rewriter(None, "#hello"), "#hello");

    assert_eq!(
      rewriter(None, "src/assets/logo.svg"),
      "/@foo/bar/1.2.3/src/assets/logo.svg"
    );

    assert_eq!(
      rewriter(
        Some(&Url::parse("file:///esm").unwrap()),
        "./src/assets/logo.svg"
      ),
      "/@foo/bar/1.2.3/./src/assets/logo.svg"
    );
  }
}
