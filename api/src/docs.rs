// Copyright 2024 the JSR authors. All rights reserved. MIT license.
use crate::db::GithubRepository;
use crate::db::RuntimeCompat;
use crate::ids::PackageName;
use crate::ids::ScopeName;
use crate::ids::Version;
use anyhow::Context;
use comrak::nodes::{Ast, AstNode, NodeValue};
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
use deno_doc::Location;
use deno_doc::{DocNode, DocNodeDef};
use deno_semver::RangeSetOrTag;
use indexmap::IndexMap;
use std::borrow::Cow;
use std::cell::RefCell;
use std::rc::Rc;
use std::sync::Arc;
use std::sync::OnceLock;
use tracing::instrument;
use url::Url;

pub type DocNodesByUrl = IndexMap<ModuleSpecifier, Vec<DocNode>>;

pub type URLRewriter =
  Arc<dyn (Fn(Option<&ShortPath>, &str) -> String) + Send + Sync>;

thread_local! {
  static CURRENT_FILE: RefCell<Option<Option<ShortPath>>> = const { RefCell::new(None) };
  static URL_REWRITER: RefCell<Option<URLRewriter>> = const { RefCell::new(None) };
}

lazy_static::lazy_static! {
  static ref AMMONIA: ammonia::Builder<'static> = {
    let mut ammonia_builder = ammonia::Builder::default();

    ammonia_builder
      .add_tags(["video", "button", "svg", "path", "rect"])
      .add_generic_attributes(["id", "align"])
      .add_tag_attributes("button", ["data-copy"])
      .add_tag_attributes(
        "svg",
        [
          "class",
          "width",
          "height",
          "viewBox",
          "fill",
          "xmlns",
          "stroke",
          "stroke-width",
          "stroke-linecap",
          "stroke-linejoin",
        ],
      )
      .add_tag_attributes(
        "path",
        [
          "d",
          "fill",
          "fill-rule",
          "clip-rule",
          "stroke",
          "stroke-width",
          "stroke-linecap",
          "stroke-linejoin",
        ],
      )
      .add_tag_attributes("rect", ["x", "y", "width", "height", "fill"])
      .add_tag_attributes("video", ["src", "controls"])
      .add_allowed_classes("pre", ["highlight"])
      .add_allowed_classes("button", ["copyButton"])
      .add_allowed_classes(
        "div",
        [
          "alert",
          "alert-note",
          "alert-tip",
          "alert-important",
          "alert-warning",
          "alert-caution",
        ],
      )
      .link_rel(Some("nofollow"))
      .url_relative(ammonia::UrlRelative::Custom(Box::new(
        AmmoniaRelativeUrlEvaluator(),
      )))
      .add_allowed_classes("span", crate::tree_sitter::CLASSES);

    ammonia_builder
  };
}

struct AmmoniaRelativeUrlEvaluator();

impl ammonia::UrlRelativeEvaluate<'_> for AmmoniaRelativeUrlEvaluator {
  fn evaluate<'a>(&self, url: &'a str) -> Option<Cow<'a, str>> {
    URL_REWRITER.with(|url_rewriter| {
      let rewriter = url_rewriter.borrow();
      let url_rewriter = rewriter.as_ref().unwrap();
      CURRENT_FILE.with(|current_file| {
        Some(
          url_rewriter(current_file.borrow().as_ref().unwrap().as_ref(), url)
            .into(),
        )
      })
    })
  }
}

enum Alert {
  Note,
  Tip,
  Important,
  Warning,
  Caution,
}

fn match_node_value<'a>(
  arena: &'a comrak::Arena<AstNode<'a>>,
  node: &'a AstNode<'a>,
  options: &comrak::Options,
  plugins: &comrak::Plugins,
) {
  match &node.data.borrow().value {
    NodeValue::BlockQuote => {
      if let Some(paragraph_child) = node.first_child() {
        if paragraph_child.data.borrow().value == NodeValue::Paragraph {
          let alert = paragraph_child.first_child().and_then(|text_child| {
            if let NodeValue::Text(text) = &text_child.data.borrow().value {
              match text
                .split_once(' ')
                .map_or((text.as_str(), None), |(kind, title)| {
                  (kind, Some(title))
                }) {
                ("[!NOTE]", title) => {
                  Some((Alert::Note, title.unwrap_or("Note").to_string()))
                }
                ("[!TIP]", title) => {
                  Some((Alert::Tip, title.unwrap_or("Tip").to_string()))
                }
                ("[!IMPORTANT]", title) => Some((
                  Alert::Important,
                  title.unwrap_or("Important").to_string(),
                )),
                ("[!WARNING]", title) => {
                  Some((Alert::Warning, title.unwrap_or("Warning").to_string()))
                }
                ("[!CAUTION]", title) => {
                  Some((Alert::Caution, title.unwrap_or("Caution").to_string()))
                }
                _ => None,
              }
            } else {
              None
            }
          });

          if let Some((alert, title)) = alert {
            let start_col = node.data.borrow().sourcepos.start;

            let document = arena.alloc(AstNode::new(RefCell::new(Ast::new(
              NodeValue::Document,
              start_col,
            ))));

            let node_without_alert = arena.alloc(AstNode::new(RefCell::new(
              Ast::new(NodeValue::Paragraph, start_col),
            )));

            for child_node in paragraph_child.children().skip(1) {
              node_without_alert.append(child_node);
            }
            for child_node in node.children().skip(1) {
              node_without_alert.append(child_node);
            }

            document.append(node_without_alert);

            let html =
              deno_doc::html::comrak::render_node(document, options, plugins);

            let alert_title = match alert {
              Alert::Note => {
                format!("{}{title}", include_str!("./docs/info-circle.svg"))
              }
              Alert::Tip => {
                format!("{}{title}", include_str!("./docs/bulb.svg"))
              }
              Alert::Important => {
                format!("{}{title}", include_str!("./docs/warning-message.svg"))
              }
              Alert::Warning => format!(
                "{}{title}",
                include_str!("./docs/warning-triangle.svg")
              ),
              Alert::Caution => {
                format!("{}{title}", include_str!("./docs/warning-octagon.svg"))
              }
            };

            let html = format!(
              r#"<div class="alert alert-{}"><div>{alert_title}</div><div>{html}</div></div>"#,
              match alert {
                Alert::Note => "note",
                Alert::Tip => "tip",
                Alert::Important => "important",
                Alert::Warning => "warning",
                Alert::Caution => "caution",
              }
            );

            let alert_node = arena.alloc(AstNode::new(RefCell::new(Ast::new(
              NodeValue::HtmlBlock(comrak::nodes::NodeHtmlBlock {
                block_type: 6,
                literal: html,
              }),
              start_col,
            ))));
            node.insert_before(alert_node);
            node.detach();
          }
        }
      }
    }
    NodeValue::Link(link) => {
      if link.url.ends_with(".mov") || link.url.ends_with(".mp4") {
        let start_col = node.data.borrow().sourcepos.start;

        let html = format!(r#"<video src="{}" controls></video>"#, link.url);

        let alert_node = arena.alloc(AstNode::new(RefCell::new(Ast::new(
          NodeValue::HtmlBlock(comrak::nodes::NodeHtmlBlock {
            block_type: 6,
            literal: html,
          }),
          start_col,
        ))));
        node.insert_before(alert_node);
        node.detach();
      }
    }
    _ => {}
  }
}

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
  exports: &crate::db::ExportsMap,
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
  github_repository: Option<GithubRepository>,
  is_readme: bool,
) -> URLRewriter {
  Arc::new(move |current_file, url| {
    if url.starts_with('#') || url.starts_with('/') {
      return url.to_string();
    }

    let base = if let Some(github_repository) = &github_repository {
      if url.rsplit_once('.').is_some_and(|(_path, extension)| {
        matches!(
          extension,
          "png"
            | "jpg"
            | "jpeg"
            | "svg"
            | "webm"
            | "webp"
            | "mp4"
            | "mov"
            | "avif"
            | "gif"
            | "ico"
        )
      }) {
        format!(
          "https://raw.githubusercontent.com/{}/{}/HEAD",
          github_repository.owner, github_repository.name
        )
      } else {
        format!(
          "https://github.com/{}/{}/blob/HEAD",
          github_repository.owner, github_repository.name
        )
      }
    } else {
      base.clone()
    };

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
  github_repository: Option<GithubRepository>,
  has_readme: bool,
  runtime_compat: RuntimeCompat,
  registry_url: String,
) -> GenerateCtx {
  let package_name = format!("@{scope}/{package}");
  let url_rewriter_base = format!("/{package_name}/{version}");

  let url_rewriter =
    get_url_rewriter(url_rewriter_base, github_repository, has_readme);

  let markdown_renderer = deno_doc::html::comrak::create_renderer(
    Some(Arc::new(super::tree_sitter::ComrakAdapter {
      show_line_numbers: false,
    })),
    Some(Box::new(match_node_value)),
    Some(Box::new(|html| AMMONIA.clean(&html).to_string())),
  );

  let markdown_renderer = Rc::new(
    move |md: &str,
          title_only: bool,
          file_path: Option<ShortPath>,
          anchorizer: deno_doc::html::jsdoc::Anchorizer| {
      CURRENT_FILE.set(Some(file_path));
      URL_REWRITER.set(Some(url_rewriter.clone()));

      // we pass None as we know that the comrak renderer doesnt use this option
      // and as such can save a clone. careful if comrak renderer changes.
      let rendered = markdown_renderer(md, title_only, None, anchorizer);

      CURRENT_FILE.set(None);
      URL_REWRITER.set(None);

      rendered
    },
  );

  GenerateCtx::new(
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
      usage_composer: (Rc::new(DocUsageComposer {
        runtime_compat,
        scope,
        package,
      })),
      rewrite_map: Some(rewrite_map),
      category_docs: None,
      disable_search: false,
      symbol_redirect_map: None,
      default_symbol_map: None,
      markdown_renderer,
      markdown_stripper: Rc::new(deno_doc::html::comrak::strip),
      head_inject: None,
    },
    None,
    deno_doc::html::FileMode::Normal,
    doc_nodes_by_url,
  )
  .unwrap()
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
  github_repository: Option<GithubRepository>,
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
    github_repository,
    readme.is_some(),
    runtime_compat,
    registry_url,
  );

  match req {
    DocsRequest::AllSymbols => {
      let render_ctx =
        RenderContext::new(&ctx, &[], UrlResolveKind::AllSymbols);

      let all_doc_nodes = ctx.doc_nodes.values().flatten().map(Cow::Borrowed);

      let partitions_by_kind =
        deno_doc::html::partition::partition_nodes_by_entrypoint(
          &ctx,
          all_doc_nodes,
          true,
        );

      let sections = deno_doc::html::namespace::render_namespace(
        partitions_by_kind.into_iter().map(|(path, nodes)| {
          (
            render_ctx.clone(),
            deno_doc::html::SectionHeaderCtx::new_for_all_symbols(
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
                title_only: false,
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

      let render_ctx = RenderContext::new(
        &ctx,
        doc_nodes,
        UrlResolveKind::File { file: short_path },
      );

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
    let mut nodes = doc_nodes
      .iter()
      .filter(|node| {
        !(matches!(node.def, DocNodeDef::ModuleDoc | DocNodeDef::Import { .. })
          || node.declaration_kind == deno_doc::node::DeclarationKind::Private)
          && node.get_name() == next_part
      })
      .flat_map(|node| {
        if let Some(reference) = node.reference_def() {
          ctx
            .resolve_reference(node.parent.as_deref(), &reference.target)
            .map(|node| node.into_owned())
            .collect::<Vec<_>>()
        } else {
          vec![node.clone()]
        }
      })
      .collect::<Vec<_>>();

    if name_parts.peek().is_some() {
      for node in &nodes {
        let drilldown_node = match &node.def {
          DocNodeDef::Class { class_def: class } => {
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
                    method.kind,
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
          DocNodeDef::Interface {
            interface_def: interface,
          } => {
            let drilldown_name =
              name_parts.clone().collect::<Vec<_>>().join(".");

            interface
              .methods
              .iter()
              .find_map(|method| {
                if method.name == drilldown_name {
                  Some(node.create_child_method(
                    DocNode::from(method.clone()),
                    true,
                    method.kind,
                  ))
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
          DocNodeDef::TypeAlias {
            type_alias_def: type_alias,
          } => {
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
                      method.kind,
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
          DocNodeDef::Variable {
            variable_def: variable,
          } => {
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
                      method.kind,
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
          DocNodeDef::Import { .. }
          | DocNodeDef::Enum { .. }
          | DocNodeDef::ModuleDoc
          | DocNodeDef::Function { .. }
          | DocNodeDef::Namespace { .. }
          | DocNodeDef::Reference { .. } => None,
        };

        if let Some(drilldown_node) = drilldown_node {
          break 'outer vec![drilldown_node];
        }
      }
    }

    nodes = nodes
      .into_iter()
      .flat_map(|node| {
        if let Some(reference) = node.reference_def() {
          ctx
            .resolve_reference(node.parent.as_deref(), &reference.target)
            .map(|node| node.into_owned())
            .collect::<Vec<_>>()
        } else {
          vec![node]
        }
      })
      .collect::<Vec<_>>();

    if name_parts.peek().is_none() {
      break nodes;
    }

    if let Some(namespace_node) = nodes
      .iter()
      .find(|node| matches!(node.def, DocNodeDef::Namespace { .. }))
    {
      namespace_paths.push(next_part.to_string());
      doc_nodes = namespace_node
        .namespace_children
        .clone()
        .unwrap()
        .into_iter()
        .flat_map(|node| {
          if let Some(reference_def) = node.reference_def() {
            ctx
              .resolve_reference(Some(namespace_node), &reference_def.target)
              .map(|node| node.into_owned())
              .collect()
          } else {
            vec![node]
          }
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
    UrlResolveKind::File { file: short_path },
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
    toc_ctx: Box::new(toc_ctx),
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
      UrlResolveKind::File { file } => format!(
        "{doc_base}{}/",
        if file.is_main {
          String::new()
        } else {
          format!("/{}", file.path)
        }
      ),
      UrlResolveKind::Category { .. } => unreachable!(),
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
          if let Some(range) = req.version_req.range() {
            if let Ok(version) = Version::new(&range.to_string()) {
              // If using a specific version, link to it (e.g. prerelease)
              version_path = Cow::Owned(format!("@{}", version));
            }
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
    let scoped_name = format!("@{}/{}", self.scope, self.package);

    let (is_main, path) = current_resolve
      .get_file()
      .map(|short_path| (short_path.is_main, &*short_path.path))
      .unwrap_or((true, ""));

    let url = format!(
      "@{}/{}{}",
      self.scope,
      self.package,
      if is_main {
        String::new()
      } else {
        format!("/{path}")
      }
    );

    let import = format!(
      "\nImport symbol\n{}",
      usage_to_md(&url, Some(self.package.as_str()))
    );

    if !self.runtime_compat.deno.is_some_and(|compat| !compat) {
      map.insert(
          UsageComposerEntry {
            name: "Deno".to_string(),
            icon: Some(
              r#"<img src="/logos/deno.svg" alt="deno logo" draggable="false" />"#.into(),
            ),
          },
          format!("Add Package\n```\ndeno add jsr:{scoped_name}\n```{import}\n---- OR ----\n\nImport directly with a jsr specifier\n{}\n", usage_to_md(&format!("jsr:{url}"), Some(self.package.as_str()))),
        );
    }

    if !self.runtime_compat.node.is_some_and(|compat| !compat) {
      map.insert(
          UsageComposerEntry {
            name: "npm".to_string(),
            icon: Some(
              r#"<img src="/logos/npm_textless.svg" alt="npm logo" draggable="false" />"#.into(),
            ),
          },
          format!("Add Package\n```\nnpx jsr add {scoped_name}\n```{import}"),
        );
      map.insert(
          UsageComposerEntry {
            name: "Yarn".to_string(),
            icon: Some(
              r#"<img src="/logos/yarn_textless.svg" alt="yarn logo" draggable="false" />"#.into(),
            ),
          },
          format!("Add Package\n```\nyarn dlx jsr add {scoped_name}\n```{import}"),
        );
      map.insert(
          UsageComposerEntry {
            name: "pnpm".to_string(),
            icon: Some(
              r#"<img src="/logos/pnpm_textless.svg" alt="pnpm logo" draggable="false" />"#.into(),
            ),
          },
          format!("Add Package\n```\npnpm dlx jsr add {scoped_name}\n```{import}"),
        );
    }

    if !self.runtime_compat.bun.is_some_and(|compat| !compat) {
      map.insert(
        UsageComposerEntry {
          name: "Bun".to_string(),
          icon: Some(
            r#"<img src="/logos/bun.svg" alt="bun logo" draggable="false" />"#
              .into(),
          ),
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
        "/@foo/bar@0.0.1/doc"
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
        "/@foo/bar@0.0.1/doc"
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
        "/@foo/bar@0.0.1/doc"
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

  #[test]
  fn test_url_rewriter() {
    let base = String::from("/@foo/bar/1.2.3");
    let rewriter = get_url_rewriter(base.clone(), None, false);

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

    let rewriter = get_url_rewriter(base.clone(), None, true);

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

    let rewriter = get_url_rewriter(
      base,
      Some(GithubRepository {
        id: 0,
        owner: "foo".to_string(),
        name: "bar".to_string(),
        updated_at: Default::default(),
        created_at: Default::default(),
      }),
      true,
    );

    assert_eq!(rewriter(None, "#hello"), "#hello");

    assert_eq!(
      rewriter(None, "src/assets/foo"),
      "https://github.com/foo/bar/blob/HEAD/src/assets/foo"
    );

    assert_eq!(
      rewriter(None, "src/assets/logo.svg"),
      "https://raw.githubusercontent.com/foo/bar/HEAD/src/assets/logo.svg"
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
      "https://raw.githubusercontent.com/foo/bar/HEAD/./src/assets/logo.svg"
    );
  }
}
