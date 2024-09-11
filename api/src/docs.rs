// Copyright 2024 the JSR authors. All rights reserved. MIT license.
use crate::db::RuntimeCompat;
use crate::ids::PackageName;
use crate::ids::ScopeName;
use crate::ids::Version;
use anyhow::Context;
use deno_ast::ModuleSpecifier;
use deno_doc::html::pages::SymbolPage;
use deno_doc::html::DocNodeWithContext;
use deno_doc::html::GenerateCtx;
use deno_doc::html::HrefResolver;
use deno_doc::html::RenderContext;
use deno_doc::html::ShortPath;
use deno_doc::html::UrlResolveKind;
use deno_doc::html::UsageComposerEntry;
use deno_doc::html::HANDLEBARS;
use deno_doc::DocNode;
use deno_doc::DocNodeKind;
use deno_doc::Location;
use deno_semver::RangeSetOrTag;
use indexmap::IndexMap;
use std::rc::Rc;
use std::sync::Arc;
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

#[derive(Debug)]
pub enum GeneratedDocsOutput {
  Docs(GeneratedDocs),
  Redirect(String),
}

#[derive(Debug)]
pub struct GeneratedDocs {
  pub breadcrumbs: Option<String>,
  pub toc: Option<String>,
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
  Arc::new(move |current_file, url| {
    if url.starts_with('#') || url.starts_with('/') {
      return url.to_string();
    }

    if !is_readme {
      if let Some(current_file) = current_file {
        let (path, _file) = current_file
          .specifier
          .path()
          .rsplit_once('/')
          .unwrap_or((current_file.specifier.path(), ""));
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
pub fn get_generate_ctx<'a>(
  doc_nodes_by_url: DocNodesByUrl,
  main_entrypoint: Option<ModuleSpecifier>,
  rewrite_map: IndexMap<ModuleSpecifier, String>,
  scope: ScopeName,
  package: PackageName,
  version: Version,
  version_is_latest: bool,
  has_readme: bool,
  runtime_compat: RuntimeCompat,
  registry_url: String,
) -> GenerateCtx {
  let package_name = format!("@{scope}/{package}");
  let url_rewriter_base = format!("/{package_name}/{version}");

  let mut generate_ctx = GenerateCtx::new(
    deno_doc::html::GenerateOptions {
      package_name: Some(package_name),
      main_entrypoint,
      href_resolver: Rc::new(DocResolver {
        scope: scope.clone(),
        package: package.clone(),
        version,
        version_is_latest,
        registry_url,
        deno_types: DENO_TYPES
          .get_or_init(|| {
            serde_json::from_str(include_str!("./docs/deno_types.json"))
              .unwrap()
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
        let scoped_name = format!("@{scope}/{package}");

        let import = format!("\nImport symbol\n{}", deno_doc::html::usage_to_md(ctx, doc_nodes, &url));

        if !runtime_compat.deno.is_some_and(|compat| !compat) {
          map.insert(
            UsageComposerEntry {
              name: "Deno".to_string(),
              icon: Some(
                r#"<img src="/logos/deno.svg" alt="deno logo" draggable={false} />"#.into(),
              ),
            },
            format!("Add Package\n```\ndeno add {scoped_name}\n```{import}\n---- OR ----\n\nImport directly with a jsr specifier\n{}\n", deno_doc::html::usage_to_md(ctx, doc_nodes, &format!("jsr:{url}"))),
          );
        }

        if !runtime_compat.node.is_some_and(|compat| !compat) {
          map.insert(
            UsageComposerEntry {
              name: "npm".to_string(),
              icon: Some(
                r#"<img src="/logos/npm_textless.svg" alt="npm logo" draggable={false} />"#.into(),
              ),
            },
            format!("Add Package\n```\nnpx jsr add {scoped_name}\n```{import}"),
          );
          map.insert(
            UsageComposerEntry {
              name: "Yarn".to_string(),
              icon: Some(
                r#"<img src="/logos/yarn_textless.svg" alt="yarn logo" draggable={false} />"#.into(),
              ),
            },
            format!("Add Package\n```\nyarn dlx jsr add {scoped_name}\n```{import}"),
          );
          map.insert(
            UsageComposerEntry {
              name: "pnpm".to_string(),
              icon: Some(
                r#"<img src="/logos/pnpm_textless.svg" alt="pnpm logo" draggable={false} />"#.into(),
              ),
            },
            format!("Add Package\n```\npnpm dlx jsr add {scoped_name}\n```{import}"),
          );
        }

        if !runtime_compat.bun.is_some_and(|compat| !compat) {
          map.insert(
            UsageComposerEntry {
              name: "Bun".to_string(),
              icon: Some(
                r#"<img src="/logos/bun.svg" alt="bun logo" draggable={false} />"#.into(),
              ),
            },
            format!("Add Package\n```\nbunx jsr add {scoped_name}\n```{import}"),
          );
        }

        map
      })),
      rewrite_map: Some(rewrite_map),
      composable_output: false,
      category_docs: None,
      disable_search: false,
      symbol_redirect_map: None,
      default_symbol_map: None,
    },
    None,
    deno_doc::html::FileMode::Normal,
    doc_nodes_by_url,
  )
  .unwrap();

  generate_ctx.url_rewriter =
    Some(get_url_rewriter(url_rewriter_base, has_readme));

  generate_ctx
}

#[allow(clippy::too_many_arguments)]
#[instrument(
  name = "generate_docs_html",
  skip(doc_nodes_by_url, rewrite_map, readme),
  err
)]
pub fn generate_docs_html(
  doc_nodes_by_url: DocNodesByUrl,
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
) -> Result<Option<GeneratedDocsOutput>, anyhow::Error> {
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
      let render_ctx =
        RenderContext::new(&ctx, &[], UrlResolveKind::AllSymbols);

      let all_doc_nodes = ctx
        .doc_nodes
        .values()
        .flatten()
        .cloned()
        .collect::<Vec<_>>();

      let partitions_by_kind =
        deno_doc::html::partition::partition_nodes_by_entrypoint(
          &all_doc_nodes,
          true,
        );

      let sections = deno_doc::html::namespace::render_namespace(
        &render_ctx,
        partitions_by_kind.into_iter().map(|(path, nodes)| {
          (
            deno_doc::html::SectionHeaderCtx::new_for_namespace(
              &render_ctx,
              &path,
            ),
            nodes,
          )
        }),
      );

      let breadcrumbs = HANDLEBARS
        .render("breadcrumbs", &render_ctx.get_breadcrumbs())
        .context("failed to render breadcrumbs")?;
      let main = HANDLEBARS
        .render(
          "symbol_content",
          &deno_doc::html::SymbolContentCtx {
            id: String::new(),
            sections,
            docs: None,
          },
        )
        .context("failed to all symbols list")?;

      Ok(Some(GeneratedDocsOutput::Docs(GeneratedDocs {
        breadcrumbs: Some(breadcrumbs),
        toc: None,
        main,
      })))
    }
    DocsRequest::Index => {
      let doc_nodes = ctx
        .main_entrypoint
        .as_ref()
        .map(|entrypoint| ctx.doc_nodes.get(entrypoint).unwrap().as_slice())
        .unwrap_or_default();

      let render_ctx =
        RenderContext::new(&ctx, doc_nodes, UrlResolveKind::Root);

      let mut index_module_doc = ctx
        .main_entrypoint
        .as_ref()
        .map(|entrypoint| {
          deno_doc::html::jsdoc::ModuleDocCtx::new(&render_ctx, entrypoint)
        })
        .unwrap_or_default();

      if index_module_doc.sections.docs.is_none() {
        let markdown = readme
          .as_ref()
          .and_then(|readme| {
            deno_doc::html::jsdoc::markdown_to_html(
              &render_ctx,
              readme,
              deno_doc::html::jsdoc::MarkdownToHTMLOptions {
                summary: false,
                summary_prefer_title: false,
                no_toc: false,
              },
            )
          })
          .unwrap_or(
            r#"<div style="font-style: italic;">No docs found.</div>"#
              .to_string(),
          );
        index_module_doc.sections.docs = Some(markdown);
      }

      let main = HANDLEBARS
        .render("module_doc", &index_module_doc)
        .context("failed to render index module doc")?;

      let toc_ctx = deno_doc::html::ToCCtx::new(render_ctx, true, Some(&[]));

      let toc = HANDLEBARS
        .render("toc", &toc_ctx)
        .context("failed to render toc")?;

      Ok(Some(GeneratedDocsOutput::Docs(GeneratedDocs {
        breadcrumbs: None,
        toc: Some(toc),
        main,
      })))
    }
    DocsRequest::File(specifier) => {
      let (short_path, doc_nodes) = ctx
        .doc_nodes
        .iter()
        .find(|(short_path, _)| short_path.specifier == specifier)
        .context("doc nodes missing for specifier")?;

      let render_ctx =
        RenderContext::new(&ctx, doc_nodes, UrlResolveKind::File(short_path));

      let module_doc =
        deno_doc::html::jsdoc::ModuleDocCtx::new(&render_ctx, short_path);

      let breadcrumbs = HANDLEBARS
        .render("breadcrumbs", &render_ctx.get_breadcrumbs())
        .context("failed to render breadcrumbs")?;

      let main = HANDLEBARS
        .render("module_doc", &module_doc)
        .context("failed to render module doc")?;

      let toc_ctx = deno_doc::html::ToCCtx::new(render_ctx, false, Some(&[]));

      let toc = HANDLEBARS
        .render("toc", &toc_ctx)
        .context("failed to render toc")?;

      Ok(Some(GeneratedDocsOutput::Docs(GeneratedDocs {
        breadcrumbs: Some(breadcrumbs),
        toc: Some(toc),
        main,
      })))
    }
    DocsRequest::Symbol(specifier, symbol) => {
      let (short_path, doc_nodes) = ctx
        .doc_nodes
        .iter()
        .find(|(short_path, _)| short_path.specifier == specifier)
        .context("doc nodes missing for specifier")?;

      let Some(symbol_page) =
        generate_symbol_page(&ctx, short_path, doc_nodes, &symbol)
      else {
        return Ok(None);
      };

      match symbol_page {
        SymbolPage::Symbol {
          breadcrumbs_ctx,
          symbol_group_ctx,
          toc_ctx,
          categories_panel: _categories_panel,
        } => {
          let breadcrumbs = HANDLEBARS
            .render("breadcrumbs", &breadcrumbs_ctx)
            .context("failed to render breadcrumbs")?;

          let main = HANDLEBARS
            .render("symbol_group", &symbol_group_ctx)
            .context("failed to render symbol group")?;

          let toc = HANDLEBARS
            .render("toc", &toc_ctx)
            .context("failed to render toc")?;

          Ok(Some(GeneratedDocsOutput::Docs(GeneratedDocs {
            breadcrumbs: Some(breadcrumbs),
            toc: Some(toc),
            main,
          })))
        }
        SymbolPage::Redirect { href, .. } => {
          Ok(Some(GeneratedDocsOutput::Redirect(href)))
        }
      }
    }
  }
}

fn generate_symbol_page(
  ctx: &GenerateCtx,
  short_path: &ShortPath,
  doc_nodes_for_module: &[DocNodeWithContext],
  name: &str,
) -> Option<SymbolPage> {
  let mut name_parts = name.split('.').peekable();
  let mut doc_nodes = doc_nodes_for_module.to_vec();
  let mut namespace_paths = vec![];

  let doc_nodes = 'outer: loop {
    let next_part = name_parts.next()?;
    let nodes = doc_nodes
      .iter()
      .filter(|node| {
        !(matches!(node.kind(), DocNodeKind::ModuleDoc | DocNodeKind::Import)
          || node.declaration_kind == deno_doc::node::DeclarationKind::Private)
          && node.get_name() == next_part
      })
      .cloned()
      .collect::<Vec<_>>();

    if name_parts.peek().is_some() {
      for node in &nodes {
        let drilldown_node =
          match node.kind() {
            DocNodeKind::Class => {
              let mut drilldown_parts = name_parts.clone().collect::<Vec<_>>();
              let mut is_static = true;

              if drilldown_parts[0] == "prototype" {
                if drilldown_parts.len() == 1 {
                  return Some(SymbolPage::Redirect {
                    current_symbol: name.to_string(),
                    href: name.rsplit_once('.').unwrap().0.to_string(),
                  });
                } else {
                  is_static = false;
                  drilldown_parts.remove(0);
                }
              }

              let drilldown_name = drilldown_parts.join(".");

              let class = node.class_def().unwrap();

              class
                .methods
                .iter()
                .find_map(|method| {
                  if *method.name == drilldown_name
                    && method.is_static == is_static
                  {
                    Some(node.create_child_method(
                      DocNode::function(
                        method.name.clone(),
                        false,
                        method.location.clone(),
                        node.declaration_kind,
                        method.js_doc.clone(),
                        method.function_def.clone(),
                      ),
                      is_static,
                    ))
                  } else {
                    None
                  }
                })
                .or_else(|| {
                  class.properties.iter().find_map(|property| {
                    if *property.name == drilldown_name
                      && property.is_static == is_static
                    {
                      Some(node.create_child_property(
                        DocNode::from(property.clone()),
                        is_static,
                      ))
                    } else {
                      None
                    }
                  })
                })
            }
            DocNodeKind::Interface => {
              let drilldown_name =
                name_parts.clone().collect::<Vec<_>>().join(".");

              let interface = node.interface_def().unwrap();

              interface
                .methods
                .iter()
                .find_map(|method| {
                  if method.name == drilldown_name {
                    Some(
                      node.create_child_method(
                        DocNode::from(method.clone()),
                        true,
                      ),
                    )
                  } else {
                    None
                  }
                })
                .or_else(|| {
                  interface.properties.iter().find_map(|property| {
                    if property.name == drilldown_name {
                      Some(node.create_child_property(
                        DocNode::from(property.clone()),
                        true,
                      ))
                    } else {
                      None
                    }
                  })
                })
            }
            DocNodeKind::TypeAlias => {
              let type_alias = node.type_alias_def().unwrap();

              if let Some(ts_type_literal) =
                type_alias.ts_type.type_literal.as_ref()
              {
                let drilldown_name =
                  name_parts.clone().collect::<Vec<_>>().join(".");

                ts_type_literal
                  .methods
                  .iter()
                  .find_map(|method| {
                    if method.name == drilldown_name {
                      Some(node.create_child_method(
                        DocNode::from(method.clone()),
                        true,
                      ))
                    } else {
                      None
                    }
                  })
                  .or_else(|| {
                    ts_type_literal.properties.iter().find_map(|property| {
                      if property.name == drilldown_name {
                        Some(node.create_child_property(
                          DocNode::from(property.clone()),
                          true,
                        ))
                      } else {
                        None
                      }
                    })
                  })
              } else {
                None
              }
            }
            DocNodeKind::Variable => {
              let variable = node.variable_def().unwrap();

              if let Some(ts_type_literal) = variable
                .ts_type
                .as_ref()
                .and_then(|ts_type| ts_type.type_literal.as_ref())
              {
                let drilldown_name =
                  name_parts.clone().collect::<Vec<_>>().join(".");

                ts_type_literal
                  .methods
                  .iter()
                  .find_map(|method| {
                    if method.name == drilldown_name {
                      Some(node.create_child_method(
                        DocNode::from(method.clone()),
                        true,
                      ))
                    } else {
                      None
                    }
                  })
                  .or_else(|| {
                    ts_type_literal.properties.iter().find_map(|property| {
                      if property.name == drilldown_name {
                        Some(node.create_child_property(
                          DocNode::from(property.clone()),
                          true,
                        ))
                      } else {
                        None
                      }
                    })
                  })
              } else {
                None
              }
            }
            DocNodeKind::Import
            | DocNodeKind::Enum
            | DocNodeKind::ModuleDoc
            | DocNodeKind::Function
            | DocNodeKind::Namespace => None,
          };

        if let Some(drilldown_node) = drilldown_node {
          break 'outer vec![drilldown_node];
        }
      }
    }

    if name_parts.peek().is_none() {
      break nodes;
    }

    if let Some(namespace_node) = nodes
      .iter()
      .find(|node| matches!(node.kind(), DocNodeKind::Namespace))
    {
      namespace_paths.push(next_part.to_string());

      let namespace = namespace_node.namespace_def().unwrap();

      let parts: Rc<[String]> = namespace_paths.clone().into();

      doc_nodes = namespace
        .elements
        .iter()
        .map(|element| {
          namespace_node.create_namespace_child(element.clone(), parts.clone())
        })
        .collect();
    } else {
      return None;
    }
  };

  if doc_nodes.is_empty() {
    return None;
  }

  let render_ctx = RenderContext::new(
    ctx,
    doc_nodes_for_module,
    UrlResolveKind::File(short_path),
  );

  let (breadcrumbs_ctx, symbol_group_ctx, toc_ctx, _category_panel) =
    deno_doc::html::pages::render_symbol_page(
      &render_ctx,
      short_path,
      name,
      &doc_nodes,
    );

  Some(SymbolPage::Symbol {
    breadcrumbs_ctx,
    symbol_group_ctx,
    toc_ctx,
    categories_panel: None,
  })
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
          if file.is_main {
            String::new()
          } else {
            format!("/{}", file.path)
          }
        )
      }
      UrlResolveKind::File(file) => format!(
        "{doc_base}{}/~/",
        if file.is_main {
          String::new()
        } else {
          format!("/{}", file.path)
        }
      ),
      UrlResolveKind::Category(_) => unreachable!(),
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
        "bun" => None,
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

          Some(format!("/{}/~/{symbol}", req.name))
        }
        _ => None,
      }
    } else {
      None
    }
  }

  fn resolve_usage(&self, current_resolve: UrlResolveKind) -> Option<String> {
    let (is_main, path) = current_resolve
      .get_file()
      .map(|short_path| (short_path.is_main, &*short_path.path))
      .unwrap_or((true, ""));

    Some(format!(
      "@{}/{}{}",
      self.scope,
      self.package,
      if is_main {
        String::new()
      } else {
        format!("/{path}")
      }
    ))
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
        "/@foo/bar@0.0.1/doc"
      );
      assert_eq!(
        resolver.resolve_path(
          UrlResolveKind::Root,
          UrlResolveKind::File(&short_path)
        ),
        "/@foo/bar@0.0.1/doc/mod/~/"
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
        "/@foo/bar@0.0.1/doc"
      );
      assert_eq!(
        resolver.resolve_path(
          UrlResolveKind::AllSymbols,
          UrlResolveKind::File(&short_path)
        ),
        "/@foo/bar@0.0.1/doc/mod/~/"
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
          UrlResolveKind::File(&short_path),
          UrlResolveKind::Root
        ),
        "/@foo/bar@0.0.1"
      );
      assert_eq!(
        resolver.resolve_path(
          UrlResolveKind::File(&short_path),
          UrlResolveKind::AllSymbols
        ),
        "/@foo/bar@0.0.1/doc"
      );
      assert_eq!(
        resolver.resolve_path(
          UrlResolveKind::File(&short_path),
          UrlResolveKind::File(&short_path)
        ),
        "/@foo/bar@0.0.1/doc/mod/~/"
      );
      assert_eq!(
        resolver.resolve_path(
          UrlResolveKind::File(&short_path),
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
        "/@foo/bar@0.0.1/doc"
      );
      assert_eq!(
        resolver.resolve_path(
          UrlResolveKind::Symbol {
            file: &short_path,
            symbol: "bar"
          },
          UrlResolveKind::File(&short_path)
        ),
        "/@foo/bar@0.0.1/doc/mod/~/"
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
        Some(&ShortPath::new(
          ModuleSpecifier::parse("file:///src/mod.ts").unwrap(),
          None,
          None,
          None,
        )),
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
        Some(&ShortPath::new(
          ModuleSpecifier::parse("file:///esm").unwrap(),
          None,
          None,
          None,
        )),
        "./src/assets/logo.svg"
      ),
      "/@foo/bar/1.2.3/./src/assets/logo.svg"
    );
  }
}
