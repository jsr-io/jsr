// Copyright 2024 the JSR authors. All rights reserved. MIT license.

use anyhow::Context;
use deno_ast::ModuleSpecifier;
use deno_doc::html::{
  DocNodeWithContext, GenerateCtx, RenderContext, ShortPath, UrlResolveKind,
};
use deno_doc::{DeclarationDef, ParseOutput, Symbol};
use indexmap::IndexMap;
use shared::ids::{PackageName, ScopeName, Version};
use shared::models::{GithubRepository, ReadmeSource, RuntimeCompat};
use std::borrow::Cow;
use std::cell::RefCell;
use std::io::Read;
use std::sync::Arc;
use std::sync::OnceLock;
use std::time::Duration;
use tracing::instrument;

use comrak::nodes::Ast;
use comrak::nodes::AstNode;
use comrak::nodes::NodeValue;
use deno_doc::html::pages::SymbolPage;
use flate2::read::GzDecoder;
use shared::docs::DocNodeCacheError;
use shared::docs::DocsRequest;
use shared::docs::GeneratedDocs;
use shared::docs::GeneratedDocsContent;
use shared::docs::GeneratedDocsOutput;
use shared::docs::SYMBOL_LISTING_LIMIT;
use shared::docs::URLRewriter;
pub use shared::docs::get_docs_info;
use shared::docs::has_create_export;
use shared::docs::serialize_doc_nodes;

/// Maximum number of concurrent doc rendering operations. Each doc render
/// builds a GenerateCtx from doc nodes, which can use 10-50 MB.
/// Without a limit, high concurrent requests can easily exceed 2 GB.
const MAX_CONCURRENT_DOC_RENDERS: usize = 16;

static DOC_RENDER_SEMAPHORE: once_cell::sync::Lazy<
  Arc<tokio::sync::Semaphore>,
> = once_cell::sync::Lazy::new(|| {
  Arc::new(tokio::sync::Semaphore::new(MAX_CONCURRENT_DOC_RENDERS))
});

/// Acquire a permit to perform doc rendering. Callers should hold the permit
/// for the duration of the render.
pub async fn acquire_doc_render_permit() -> tokio::sync::OwnedSemaphorePermit {
  DOC_RENDER_SEMAPHORE
    .clone()
    .acquire_owned()
    .await
    .expect("doc render semaphore closed")
}

/// Upper bound on the number of symbol rows the "all symbols" listing may
/// contain across *every* entrypoint.
///
/// [`SYMBOL_LISTING_LIMIT`] already bounds each module's own listing, but the
/// all-symbols listing concatenates one per entrypoint, so a package with many
/// entrypoints multiplies straight past it: `@ubx/sdk-aws` publishes 1921
/// exports, putting its ceiling near 3.9 million rows. The resulting response
/// exceeded Cloud Run's 32 MiB cap, so every request built the whole thing,
/// spent ~1.6s of CPU, and was then killed mid-flight — 871 times in one hour
/// from a single crawler, and never cacheable because a 500 is not.
///
/// Refusing up front turns that into a cheap, cacheable error. The durable fix
/// is for the limit in deno_doc to be a total budget rather than a per-module
/// one, at which point this guard should stop firing on its own.
const ALL_SYMBOLS_LISTING_LIMIT: usize = 10_000;

/// Whether the all-symbols listing for `ctx` would exceed
/// [`ALL_SYMBOLS_LISTING_LIMIT`].
///
/// Mirrors what rendering will actually emit: each entrypoint contributes at
/// most [`SYMBOL_LISTING_LIMIT`] rows. Counting top-level doc nodes can only
/// over-estimate a module's rendered rows, so this errs towards refusing.
pub fn all_symbols_listing_too_large(ctx: &GenerateCtx) -> bool {
  listing_rows_exceed_limit(ctx.doc_nodes.values().map(|nodes| nodes.len()))
}

/// The counting half of [`all_symbols_listing_too_large`], split out so the
/// per-entrypoint accumulation can be tested without building a `GenerateCtx`.
fn listing_rows_exceed_limit(
  module_symbol_counts: impl Iterator<Item = usize>,
) -> bool {
  let mut total = 0usize;
  for count in module_symbol_counts {
    total = total.saturating_add(count.min(SYMBOL_LISTING_LIMIT));
    if total > ALL_SYMBOLS_LISTING_LIMIT {
      return true;
    }
  }
  false
}

thread_local! {
  static CURRENT_FILE: RefCell<Option<Option<ShortPath>>> = const { RefCell::new(None) };
  static URL_REWRITER: RefCell<Option<URLRewriter>> = const { RefCell::new(None) };
}

lazy_static::lazy_static! {
  static ref AMMONIA: ammonia::Builder<'static> = {
    let mut ammonia_builder = ammonia::Builder::default();

    ammonia_builder
      .add_tags([
        "video", "button", "svg", "path", "rect", "section", "g", "defs",
        "clipPath",
      ])
      .add_generic_attributes(["id", "align"])
      .add_tag_attributes("g", ["clip-path", "fill", "opacity", "transform"])
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
      // comrak footnote output
      .add_allowed_classes("section", ["footnotes"])
      .add_allowed_classes("sup", ["footnote-ref"])
      .add_allowed_classes("a", ["footnote-backref", "anchor"])
      // deno_doc heading permalinks
      .add_tag_attributes("a", ["aria-label", "tabindex"])
      .add_allowed_classes("h1", ["anchorable"])
      .add_allowed_classes("h2", ["anchorable"])
      .add_allowed_classes("h3", ["anchorable"])
      .add_allowed_classes("h4", ["anchorable"])
      .add_allowed_classes("h5", ["anchorable"])
      .add_allowed_classes("h6", ["anchorable"])
      .add_allowed_classes(
        "div",
        [
          "alert",
          "alert-note",
          "alert-tip",
          "alert-important",
          "alert-warning",
          "alert-caution",
          "or-bar"
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
  arena: &'a comrak::Arena<'a>,
  node: &'a AstNode<'a>,
  options: &comrak::Options,
  plugins: &comrak::options::Plugins,
) {
  match &node.data.borrow().value {
    NodeValue::BlockQuote => {
      if let Some(paragraph_child) = node.first_child()
        && paragraph_child.data.borrow().value == NodeValue::Paragraph
      {
        let alert = paragraph_child.first_child().and_then(|text_child| {
          if let NodeValue::Text(text) = &text_child.data.borrow().value {
            match text
              .split_once(' ')
              .map_or((text.as_ref(), None), |(kind, title)| {
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
            Alert::Warning => {
              format!("{}{title}", include_str!("./docs/warning-triangle.svg"))
            }
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
    NodeValue::Link(link)
      if (link.url.ends_with(".mov") || link.url.ends_with(".mp4")) =>
    {
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
    _ => {}
  }
}

static DENO_TYPES: OnceLock<Arc<std::collections::HashSet<Vec<String>>>> =
  OnceLock::new();
static WEB_TYPES: OnceLock<
  Arc<std::collections::HashMap<Vec<String>, String>>,
> = OnceLock::new();

/// Deserialize doc nodes from gzip-compressed MessagePack (v2 format).
fn deserialize_doc_nodes_v2(
  bytes: &[u8],
) -> Result<ParseOutput, DocNodeCacheError> {
  let mut decoder = GzDecoder::new(bytes);
  let mut decompressed = Vec::new();
  decoder
    .read_to_end(&mut decompressed)
    .map_err(DocNodeCacheError::Decompress)?;
  let stored: shared::docs::StoredDocNodes =
    rmp_serde::from_slice(&decompressed)
      .map_err(|e| DocNodeCacheError::Deserialize(e.to_string()))?;
  if stored.version != shared::docs::DOC_NODES_VERSION {
    return Err(DocNodeCacheError::UnsupportedVersion(stored.version));
  }
  Ok(stored.doc_nodes)
}

/// Deserialize doc nodes from legacy JSON (v1 format), migrating to v2.
fn deserialize_doc_nodes_v1(
  bytes: &[u8],
) -> Result<ParseOutput, DocNodeCacheError> {
  let value: serde_json::Value = serde_json::from_slice(bytes)
    .map_err(|e| DocNodeCacheError::Deserialize(e.to_string()))?;

  match value {
    serde_json::Value::Object(map) => {
      let mut result = IndexMap::new();
      for (key, val) in map {
        let specifier = ModuleSpecifier::parse(&key)
          .map_err(DocNodeCacheError::InvalidSpecifier)?;
        result.insert(specifier, deno_doc::docnodes_v1_to_v2(val));
      }
      Ok(result)
    }
    _ => Err(DocNodeCacheError::UnexpectedJsonShape),
  }
}

/// Download doc nodes from GCS for a package version, trying v2
/// (msgpack+gzip) first and falling back to v1 (JSON) with migration.
/// Used by the diff endpoint which needs raw ParseOutput.
pub async fn download_doc_nodes(
  scope: &ScopeName,
  package: &PackageName,
  version: &Version,
  bucket: &shared::s3::Buckets,
) -> Result<Option<ParseOutput>, DocNodeCacheError> {
  Ok(
    download_doc_nodes_sized(scope, package, version, bucket)
      .await?
      .map(|(doc_nodes, _)| doc_nodes),
  )
}

/// Like [`download_doc_nodes`], but also reports how many stored bytes the
/// nodes were decoded from. [`GenerateCtxCache`] weighs its entries by that
/// figure, so it needs the size the download already knows.
async fn download_doc_nodes_sized(
  scope: &ScopeName,
  package: &PackageName,
  version: &Version,
  bucket: &shared::s3::Buckets,
) -> Result<Option<(ParseOutput, usize)>, DocNodeCacheError> {
  let v2_path = shared::s3_paths::docs_v2_path(scope, package, version);
  let v2_result = bucket
    .docs_bucket
    .download(Arc::from(v2_path.as_str()))
    .await?;

  if let Some(bytes) = v2_result {
    return Ok(Some((deserialize_doc_nodes_v2(&bytes)?, bytes.len())));
  }

  let v1_path = shared::s3_paths::docs_v1_path(scope, package, version);
  let v1_result = bucket
    .docs_bucket
    .download(Arc::from(v1_path.as_str()))
    .await?;

  let Some(bytes) = v1_result else {
    return Ok(None);
  };

  let doc_nodes = deserialize_doc_nodes_v1(&bytes)?;
  let stored_bytes = bytes.len();

  // Best-effort migration: re-upload as v2 and delete v1. Failures are
  // logged but not propagated — the doc nodes were already read successfully.
  let v2_bytes = serialize_doc_nodes(&doc_nodes);
  match bucket
    .docs_bucket
    .upload(
      Arc::from(v2_path.as_str()),
      shared::s3::UploadTaskBody::Bytes(v2_bytes),
      shared::s3::S3UploadOptions {
        content_type: Some("application/x-msgpack".into()),
        cache_control: Some(shared::s3::CACHE_CONTROL_IMMUTABLE.into()),
        gzip_encoded: true,
      },
    )
    .await
  {
    Ok(()) => {
      if let Err(err) = bucket
        .docs_bucket
        .delete_file(Arc::from(v1_path.as_str()))
        .await
      {
        tracing::warn!("failed to delete v1 doc nodes after migration: {err}");
      }
    }
    Err(err) => {
      tracing::warn!("failed to upload v2 doc nodes during migration: {err}");
    }
  }

  Ok(Some((doc_nodes, stored_bytes)))
}

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
    has_readme,
    runtime_compat,
    has_create_export,
    registry_url,
    diff
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
  github_repository: Option<GithubRepository>,
  has_readme: bool,
  runtime_compat: RuntimeCompat,
  has_create_export: bool,
  registry_url: String,
  diff: Option<(deno_doc::diff::DocDiff, bool)>,
) -> GenerateCtx {
  let package_name = format!("@{scope}/{package}");
  let url_rewriter_base = format!("/{package_name}/{version}");

  let url_rewriter =
    get_url_rewriter(url_rewriter_base, github_repository, has_readme);

  let markdown_renderer = deno_doc::html::comrak::create_renderer(
    Some(Arc::new(super::tree_sitter::ComrakAdapter::new(false))),
    Some(Box::new(match_node_value)),
    Some(Box::new(|html| AMMONIA.clean(&html).to_string())),
  );

  let markdown_renderer = Arc::new(
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

  let deno_types = DENO_TYPES
    .get_or_init(|| {
      Arc::new(
        serde_json::from_str(include_str!("./docs/deno_types.json")).unwrap(),
      )
    })
    .clone();
  let web_types = WEB_TYPES
    .get_or_init(|| {
      Arc::new(
        serde_json::from_str::<Vec<shared::docs::WebType>>(include_str!(
          "./docs/web_builtins.json"
        ))
        .unwrap()
        .into_iter()
        .map(|web_type| (web_type.id, web_type.docs))
        .collect(),
      )
    })
    .clone();

  shared::docs::get_generate_ctx(
    doc_base,
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
    web_types,
    markdown_renderer,
  )
}

#[instrument(name = "render_docs_html", skip(ctx, readme), err)]
pub fn render_docs_html(
  ctx: &GenerateCtx,
  req: DocsRequest,
  readme: Option<String>,
  readme_source: ReadmeSource,
) -> Result<Option<GeneratedDocsOutput>, anyhow::Error> {
  match req {
    DocsRequest::AllSymbols => {
      let render_ctx = RenderContext::new(ctx, &[], UrlResolveKind::AllSymbols);

      let all_symbols = deno_doc::html::AllSymbolsCtx::new(&render_ctx);
      let breadcrumbs = render_ctx.get_breadcrumbs();

      let toc = deno_doc::html::ToCCtx::new(render_ctx, false, Some(None));

      Ok(Some(GeneratedDocsOutput::Docs(GeneratedDocs {
        breadcrumbs: Some(breadcrumbs),
        toc,
        main: GeneratedDocsContent::AllSymbols(all_symbols),
      })))
    }
    DocsRequest::Index => {
      let doc_nodes = ctx
        .main_entrypoint
        .as_ref()
        .map(|entrypoint| ctx.doc_nodes.get(entrypoint).unwrap().as_slice())
        .unwrap_or_default();

      let render_ctx = RenderContext::new(ctx, doc_nodes, UrlResolveKind::Root);

      let mut index_module_doc = ctx
        .main_entrypoint
        .as_ref()
        .map(|entrypoint| {
          deno_doc::html::jsdoc::ModuleDocCtx::new(
            &render_ctx,
            entrypoint,
            false,
            false,
          )
        })
        .unwrap_or_default();

      // When the README is the overview's prose, drop the module JSDoc body
      // so the README injection below kicks in — but keep the sections, which
      // hold the module's `@example`s.
      if matches!(readme_source, ReadmeSource::Readme) {
        index_module_doc.sections.docs = None;
      }

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

      let toc = deno_doc::html::ToCCtx::new(render_ctx, true, Some(None));

      Ok(Some(GeneratedDocsOutput::Docs(GeneratedDocs {
        breadcrumbs: None,
        toc,
        main: GeneratedDocsContent::Index(index_module_doc),
      })))
    }
    DocsRequest::File(specifier) => {
      let (short_path, doc_nodes) = ctx
        .doc_nodes
        .iter()
        .find(|(short_path, _)| short_path.specifier == specifier)
        .context("doc nodes missing for specifier")?;

      let render_ctx = RenderContext::new(
        ctx,
        doc_nodes,
        UrlResolveKind::File { file: short_path },
      );

      let mut module_doc = deno_doc::html::jsdoc::ModuleDocCtx::new(
        &render_ctx,
        short_path,
        true,
        false,
      );
      if short_path.is_main {
        module_doc.sections.docs = None;
      }

      let breadcrumbs = render_ctx.get_breadcrumbs();

      let toc = deno_doc::html::ToCCtx::new(render_ctx, false, Some(None));

      Ok(Some(GeneratedDocsOutput::Docs(GeneratedDocs {
        breadcrumbs: Some(breadcrumbs),
        toc,
        main: GeneratedDocsContent::File(module_doc),
      })))
    }
    DocsRequest::Symbol(specifier, symbol) => {
      let (short_path, doc_nodes) = ctx
        .doc_nodes
        .iter()
        .find(|(short_path, _)| short_path.specifier == specifier)
        .context("doc nodes missing for specifier")?;

      let Some(symbol_page) =
        generate_symbol_page(ctx, short_path, doc_nodes, &symbol)
      else {
        return Ok(None);
      };

      match symbol_page {
        SymbolPage::Symbol {
          breadcrumbs_ctx,
          symbol_group_ctx,
          toc_ctx,
          categories_panel: _categories_panel,
        } => Ok(Some(GeneratedDocsOutput::Docs(GeneratedDocs {
          breadcrumbs: Some(breadcrumbs_ctx),
          toc: *toc_ctx,
          main: GeneratedDocsContent::Symbol(symbol_group_ctx),
        }))),
        SymbolPage::Redirect { href, .. } => {
          Ok(Some(GeneratedDocsOutput::Redirect(href)))
        }
      }
    }
  }
}

/// Resolves the references of a doc node, keeping any non-reference
/// declarations alongside the resolved targets so they are not lost (e.g. an
/// interface merged with a same-named function re-exported through
/// `export *`).
fn resolve_doc_node_references(
  ctx: &GenerateCtx,
  node: DocNodeWithContext,
) -> Vec<DocNodeWithContext> {
  if !node
    .declarations
    .iter()
    .any(|decl| decl.reference_def().is_some())
  {
    return vec![node];
  }

  let mut nodes = Vec::new();
  for decl in &node.declarations {
    if let Some(reference) = decl.reference_def() {
      nodes.extend(
        ctx
          .resolve_reference(node.parent.as_deref(), &reference.target)
          .map(|resolved| resolved.into_owned()),
      );
    }
  }

  let non_reference_decls = node
    .declarations
    .iter()
    .filter(|decl| decl.reference_def().is_none())
    .cloned()
    .collect::<Vec<_>>();
  if !non_reference_decls.is_empty() {
    let mut stripped = node;
    std::sync::Arc::make_mut(&mut stripped.inner).declarations =
      non_reference_decls;
    nodes.push(stripped);
  }

  nodes
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
    // Symbols that aren't exported get a page too: the public API refers to
    // them, so their signatures link here and this page is what those links
    // point at (jsr-io/jsr#165). deno_doc keeps them out of the listings and
    // of symbol search, since they can't be imported.
    //
    // `@internal` is different: the author asked for the symbol to be hidden,
    // so it gets no page, and deno_doc emits no link to one either.
    let mut nodes = doc_nodes
      .iter()
      .filter(|node| !node.is_hidden() && node.get_name() == next_part)
      .flat_map(|node| resolve_doc_node_references(ctx, node.clone()))
      .collect::<Vec<_>>();

    if name_parts.peek().is_some() {
      for node in &nodes {
        let declaration_kind = node.inner.declarations[0].declaration_kind;
        let drilldown_node =
          node.declarations.iter().find_map(|decl| match &decl.def {
            DeclarationDef::Class(class) => {
              let mut drilldown_parts = name_parts.clone().collect::<Vec<_>>();
              let mut is_static = true;

              if drilldown_parts[0] == "prototype" {
                if drilldown_parts.len() == 1 {
                  return Some(Err(SymbolPage::Redirect {
                    current_symbol: name.to_string(),
                    href: name.rsplit_once('.').unwrap().0.to_string(),
                    diff_status: None, // TODO
                  }));
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
                    Some(Ok(node.create_child_method(
                      Symbol::function(
                        method.name.clone(),
                        false,
                        method.location.clone(),
                        declaration_kind,
                        method.js_doc.clone(),
                        method.function_def.clone(),
                      ),
                      is_static,
                      method.kind,
                    )))
                  } else {
                    None
                  }
                })
                .or_else(|| {
                  class.properties.iter().find_map(|property| {
                    if *property.name == drilldown_name
                      && property.is_static == is_static
                    {
                      Some(Ok(node.create_child_property(
                        Symbol::from(property.clone()),
                        is_static,
                      )))
                    } else {
                      None
                    }
                  })
                })
            }
            DeclarationDef::Interface(interface) => {
              let drilldown_name =
                name_parts.clone().collect::<Vec<_>>().join(".");

              interface
                .methods
                .iter()
                .find_map(|method| {
                  if method.name == drilldown_name {
                    Some(Ok(node.create_child_method(
                      Symbol::from(method.clone()),
                      true,
                      method.kind,
                    )))
                  } else {
                    None
                  }
                })
                .or_else(|| {
                  interface.properties.iter().find_map(|property| {
                    if property.name == drilldown_name {
                      Some(Ok(node.create_child_property(
                        Symbol::from(property.clone()),
                        true,
                      )))
                    } else {
                      None
                    }
                  })
                })
            }
            DeclarationDef::TypeAlias(type_alias) => {
              if let deno_doc::ts_type::TsTypeDefKind::TypeLiteral(
                ts_type_literal,
              ) = &type_alias.ts_type.kind
              {
                let drilldown_name =
                  name_parts.clone().collect::<Vec<_>>().join(".");

                ts_type_literal
                  .methods
                  .iter()
                  .find_map(|method| {
                    if method.name == drilldown_name {
                      Some(Ok(node.create_child_method(
                        Symbol::from(method.clone()),
                        true,
                        method.kind,
                      )))
                    } else {
                      None
                    }
                  })
                  .or_else(|| {
                    ts_type_literal.properties.iter().find_map(|property| {
                      if property.name == drilldown_name {
                        Some(Ok(node.create_child_property(
                          Symbol::from(property.clone()),
                          true,
                        )))
                      } else {
                        None
                      }
                    })
                  })
              } else {
                None
              }
            }
            DeclarationDef::Variable(variable) => {
              if let Some(deno_doc::ts_type::TsTypeDefKind::TypeLiteral(
                ts_type_literal,
              )) = variable.ts_type.as_ref().map(|ts_type| &ts_type.kind)
              {
                let drilldown_name =
                  name_parts.clone().collect::<Vec<_>>().join(".");

                ts_type_literal
                  .methods
                  .iter()
                  .find_map(|method| {
                    if method.name == drilldown_name {
                      Some(Ok(node.create_child_method(
                        Symbol::from(method.clone()),
                        true,
                        method.kind,
                      )))
                    } else {
                      None
                    }
                  })
                  .or_else(|| {
                    ts_type_literal.properties.iter().find_map(|property| {
                      if property.name == drilldown_name {
                        Some(Ok(node.create_child_property(
                          Symbol::from(property.clone()),
                          true,
                        )))
                      } else {
                        None
                      }
                    })
                  })
              } else {
                None
              }
            }
            DeclarationDef::Enum(..)
            | DeclarationDef::Function(..)
            | DeclarationDef::Namespace(..)
            | DeclarationDef::Reference(..) => None,
          });

        if let Some(drilldown_result) = drilldown_node {
          match drilldown_result {
            Ok(node) => break 'outer vec![node],
            Err(redirect) => return Some(redirect),
          }
        }
      }
    }

    nodes = nodes
      .into_iter()
      .flat_map(|node| resolve_doc_node_references(ctx, node))
      .collect::<Vec<_>>();

    if name_parts.peek().is_none() {
      break nodes;
    }

    {
      let namespace_node = nodes.iter().find(|node| {
        node
          .declarations
          .iter()
          .any(|decl| matches!(decl.def, DeclarationDef::Namespace(..)))
      })?;
      namespace_paths.push(next_part.to_string());
      doc_nodes = namespace_node
        .namespace_children
        .as_ref()
        .unwrap()
        .iter()
        .flat_map(|node| {
          if let Some(reference_def) = node
            .declarations
            .iter()
            .find_map(|decl| decl.reference_def())
          {
            ctx
              .resolve_reference(Some(namespace_node), &reference_def.target)
              .map(|node| node.into_owned())
              .collect()
          } else {
            vec![node.clone()]
          }
        })
        .collect();
    }
  };

  if doc_nodes.is_empty() {
    return None;
  }

  // One symbol name can produce multiple doc nodes (e.g. a resolved
  // re-export alongside a same-named interface merged through `export *`);
  // combine their declarations so the page shows all of them. Prefer the
  // node carrying the requested name as the base so the page keeps it.
  let doc_node = if doc_nodes.len() == 1 {
    doc_nodes[0].clone()
  } else {
    let requested_name = name.rsplit('.').next().unwrap();
    let mut merged = doc_nodes
      .iter()
      .find(|node| node.get_name() == requested_name)
      .unwrap_or(&doc_nodes[0])
      .clone();
    let mut declarations = Vec::new();
    let mut seen = std::collections::HashSet::new();
    for node in &doc_nodes {
      for decl in &node.declarations {
        if seen.insert((decl.location.clone(), decl.def.to_kind())) {
          declarations.push(decl.clone());
        }
      }
    }
    std::sync::Arc::make_mut(&mut merged.inner).declarations = declarations;
    merged
  };

  let render_ctx = RenderContext::new(
    ctx,
    doc_nodes_for_module,
    UrlResolveKind::File { file: short_path },
  );

  let (breadcrumbs_ctx, symbol_group_ctx, toc_ctx, _category_panel) =
    deno_doc::html::pages::render_symbol_page(
      &render_ctx,
      short_path,
      &doc_node,
    );

  Some(SymbolPage::Symbol {
    breadcrumbs_ctx,
    symbol_group_ctx,
    toc_ctx: Box::new(toc_ctx),
    categories_panel: None,
  })
}

/// Renders a standalone markdown file from a package (e.g. a non-README `.md`
/// opened in the source view) to sanitized HTML, without building a full
/// `GenerateCtx`. `path` is the package-root-relative path of the file (with
/// a leading slash); relative links resolve against the file's directory, the
/// same way README links do.
pub fn render_markdown_file(
  md: &str,
  scope: &ScopeName,
  package: &PackageName,
  version: &Version,
  github_repository: Option<GithubRepository>,
  path: &str,
) -> Option<String> {
  let renderer = deno_doc::html::comrak::create_renderer(
    Some(Arc::new(super::tree_sitter::ComrakAdapter::new(false))),
    Some(Box::new(match_node_value)),
    Some(Box::new(|html| AMMONIA.clean(&html).to_string())),
  );

  let url_rewriter = get_url_rewriter(
    format!("/@{scope}/{package}/{version}"),
    github_repository,
    false,
  );

  let specifier = ModuleSpecifier::parse(&format!("file://{path}")).ok()?;
  let short_path = ShortPath::new(specifier, None, None, None);

  let anchorizer: deno_doc::html::jsdoc::Anchorizer = {
    let seen = std::sync::Mutex::new(std::collections::HashMap::new());
    Arc::new(move |content: String, level: u8| {
      let mut slug = String::new();
      for ch in content.chars() {
        if ch.is_alphanumeric() {
          slug.extend(ch.to_lowercase());
        } else if (ch.is_whitespace() || ch == '-') && !slug.ends_with('-') {
          slug.push('-');
        }
      }
      let mut seen = seen.lock().unwrap();
      let count = seen
        .entry((slug.clone(), level))
        .and_modify(|count| *count += 1)
        .or_insert(0);
      if *count > 0 {
        format!("{slug}-{count}")
      } else {
        slug
      }
    })
  };

  CURRENT_FILE.set(Some(Some(short_path)));
  URL_REWRITER.set(Some(url_rewriter));

  let rendered = renderer(md, false, None, anchorizer);

  CURRENT_FILE.set(None);
  URL_REWRITER.set(None);

  rendered
}

fn get_url_rewriter(
  base: String,
  github_repository: Option<GithubRepository>,
  is_readme: bool,
) -> URLRewriter {
  Arc::new(move |current_file, url| {
    // Anchors and protocol-relative URLs (`//host/...`) are left untouched.
    if url.starts_with('#') || url.starts_with("//") {
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

    // Root-relative URLs (a single leading `/`) resolve against the
    // repository root rather than the current file's directory. Without this
    // they would be served relative to jsr.io and 404 (see #768).
    if let Some(path) = url.strip_prefix('/') {
      return format!("{base}/{path}");
    }

    if !is_readme && let Some(current_file) = current_file {
      let (path, _file) = current_file
        .specifier
        .path()
        .rsplit_once('/')
        .unwrap_or((current_file.specifier.path(), ""));
      return format!("{base}{path}/{url}");
    }

    format!("{base}/{url}")
  })
}

/// Total weight the cache may hold, measured in the *stored* (gzipped
/// MessagePack) size of the doc nodes each context was built from.
///
/// This replaced a flat 64-entry capacity. Counting entries meant 64 tiny
/// packages evicted each other exactly as readily as 64 enormous ones, while
/// the memory actually held was bounded by nothing at all. Weighing by stored
/// size bounds memory instead, which both caps the worst case and lets far more
/// of the long tail stay resident — a production sample saw 534 distinct
/// packages across 2000 docs requests against those 64 slots.
///
/// Stored size is a proxy: the built context is roughly an order of magnitude
/// larger in memory than the bytes it was decoded from. The budget is set
/// conservatively for that reason, and pinning the real ratio needs cache
/// instrumentation that does not exist yet.
const MAX_CACHED_DOC_NODE_BYTES: u64 = 16 * 1024 * 1024;

/// Release contexts that go unread for this long, so a burst across many
/// packages does not pin memory for the rest of the process's life.
const CACHED_CTX_TIME_TO_IDLE: Duration = Duration::from_secs(30 * 60);

#[derive(Clone)]
struct CachedCtx {
  ctx: Arc<GenerateCtx>,
  /// Stored size of the doc nodes this was built from; the cache weight.
  stored_bytes: u32,
}

/// Loader failure. `Absent` never reaches a caller — returning it as an error
/// is how a package with no doc nodes escapes `try_get_with` without moka
/// caching the miss.
#[derive(Debug, thiserror::Error)]
enum CtxLoadError {
  #[error(transparent)]
  DocNodes(#[from] DocNodeCacheError),
  #[error("no doc nodes stored for this version")]
  Absent,
}

#[derive(Debug, thiserror::Error)]
#[error(transparent)]
pub struct GenerateCtxCacheError(Arc<CtxLoadError>);

/// Cache for fully-built GenerateCtx. Keyed by
/// `scope/package/version/is_latest/has_readme/runtime_compat` so concurrent
/// requests for the same doc page share a single GenerateCtx without
/// rebuilding.
#[derive(Clone)]
pub struct GenerateCtxCache {
  cache: moka::future::Cache<String, CachedCtx>,
}

impl GenerateCtxCache {
  pub fn new() -> Self {
    Self {
      cache: moka::future::Cache::builder()
        .max_capacity(MAX_CACHED_DOC_NODE_BYTES)
        .weigher(|_key, cached: &CachedCtx| cached.stored_bytes)
        .time_to_idle(CACHED_CTX_TIME_TO_IDLE)
        .build(),
    }
  }

  #[allow(clippy::too_many_arguments)]
  pub async fn get(
    &self,
    scope: &ScopeName,
    package: &PackageName,
    version: &Version,
    version_is_latest: bool,
    has_readme: bool,
    exports: &shared::models::ExportsMap,
    github_repository: Option<GithubRepository>,
    runtime_compat: RuntimeCompat,
    registry_url: &str,
    bucket: &shared::s3::Buckets,
  ) -> Result<Option<Arc<GenerateCtx>>, GenerateCtxCacheError> {
    // runtime_compat is part of the key because the usage instructions baked
    // into the GenerateCtx depend on it, and it is editable in the package
    // settings.
    let key = format!(
      "@{scope}/{package}/{version}/{version_is_latest}/{has_readme}/{:?}",
      runtime_compat
    );

    // `try_get_with` single-flights the miss: concurrent requests for the same
    // cold key wait on one download and one context build instead of each
    // paying both. That matters more here than for a typical cache, because
    // building a context allocates the 10-50 MB the render permit exists to
    // bound, so a stampede on a popular cold package multiplied real memory as
    // well as CPU.
    let loaded = self
      .cache
      .try_get_with(key, async {
        let Some((doc_nodes, stored_bytes)) =
          download_doc_nodes_sized(scope, package, version, bucket).await?
        else {
          return Err(CtxLoadError::Absent);
        };

        let docs_info = get_docs_info(exports, None);
        let ctx = get_generate_ctx(
          "/doc".to_string(),
          doc_nodes,
          docs_info.main_entrypoint,
          docs_info.rewrite_map,
          scope.clone(),
          package.clone(),
          version.clone(),
          version_is_latest,
          github_repository,
          has_readme,
          runtime_compat,
          has_create_export(exports),
          registry_url.to_string(),
          None,
        );

        Ok(CachedCtx {
          ctx: Arc::new(ctx),
          stored_bytes: stored_bytes.try_into().unwrap_or(u32::MAX),
        })
      })
      .await;

    match loaded {
      Ok(cached) => Ok(Some(cached.ctx)),
      Err(err) if matches!(&*err, CtxLoadError::Absent) => Ok(None),
      Err(err) => Err(GenerateCtxCacheError(err)),
    }
  }
}

#[cfg(test)]
mod tests {
  use super::*;
  use deno_ast::ModuleSpecifier;
  use deno_doc::html::ShortPath;
  use shared::ids::{PackageName, ScopeName, Version};
  use shared::models::GithubRepository;

  #[test]
  fn one_module_under_the_per_module_limit_is_allowed() {
    // The common case: a single entrypoint, however symbol-heavy, is capped by
    // SYMBOL_LISTING_LIMIT and must still render.
    assert!(!listing_rows_exceed_limit(std::iter::once(usize::MAX)));
  }

  #[test]
  fn many_small_entrypoints_can_exceed_the_total_limit() {
    // The regression this guard exists for. Every module here is far under
    // SYMBOL_LISTING_LIMIT, so the per-module cap never trims anything, yet the
    // all-symbols listing concatenates one per entrypoint. @ubx/sdk-aws
    // publishes 1921 exports; at 100 symbols each that is 192_100 rows, which
    // is what pushed the response past Cloud Run's 32 MiB cap.
    assert!(listing_rows_exceed_limit(std::iter::repeat_n(100, 1921)));
  }

  #[test]
  fn a_package_at_the_limit_is_still_served() {
    // Exactly at the budget is allowed; only exceeding it refuses.
    assert!(!listing_rows_exceed_limit(std::iter::repeat_n(
      1,
      ALL_SYMBOLS_LISTING_LIMIT
    )));
    assert!(listing_rows_exceed_limit(std::iter::repeat_n(
      1,
      ALL_SYMBOLS_LISTING_LIMIT + 1
    )));
  }

  #[test]
  fn renders_heading_permalinks_through_the_sanitizer() {
    // deno_doc emits heading permalinks as an `anchorable` heading wrapping an
    // `anchor` link around its bundled link.svg. The allowlist has to keep the
    // classes the CSS hangs off of, and it has to keep <defs>/<clipPath> so the
    // clip rect stays inside a definition instead of painting over the icon
    // (#1530).
    let html = render_markdown_file(
      "## Install\n",
      &ScopeName::new("scope".to_string()).unwrap(),
      &PackageName::new("pkg".to_string()).unwrap(),
      &Version::new("1.0.0").unwrap(),
      None,
      "/README.md",
    )
    .unwrap();

    assert!(
      html.contains(r#"<h2 id="install" class="anchorable">"#),
      "{html}"
    );
    assert!(html.contains(r##"href="#install""##), "{html}");
    assert!(html.contains(r#"class="anchor""#), "{html}");
    assert!(html.contains(r#"aria-label="Anchor""#), "{html}");
    assert!(html.contains(r#"tabindex="-1""#), "{html}");
    // the icon's clip plumbing survives, so the white rect stays unpainted
    assert!(html.contains("<g clip-path="), "{html}");
    assert!(html.contains("<defs>"), "{html}");
    assert!(html.contains("<clipPath "), "{html}");
    let (_, after_rect) = html.split_once("<rect ").expect("{html}");
    assert!(
      after_rect
        .split("</svg>")
        .next()
        .unwrap()
        .contains("</clipPath>"),
      "the clip rect escaped its <clipPath>: {html}"
    );
  }

  #[test]
  fn render_markdown_file_test() {
    let html = render_markdown_file(
      "# Hi\n\nSee the [guide](./other.md) and ![img](image.png)\n\n```ts\nconst x: number = 1;\n```\n",
      &ScopeName::new("scope".to_string()).unwrap(),
      &PackageName::new("pkg".to_string()).unwrap(),
      &Version::new("1.0.0").unwrap(),
      None,
      "/docs/guide.md",
    )
      .unwrap();
    assert!(
      html.contains(r#"href="/@scope/pkg/1.0.0/docs/./other.md""#),
      "{html}"
    );
    assert!(
      html.contains(r#"src="/@scope/pkg/1.0.0/docs/image.png""#),
      "{html}"
    );
    // code fences go through the tree-sitter highlighter
    assert!(html.contains("<span class="), "{html}");
  }

  #[test]
  fn renders_footnotes_through_the_sanitizer() {
    // goes through the real render path, so this keeps up with whatever markup
    // the deno_doc of the day emits rather than a hand-written fixture
    let html = render_markdown_file(
      "claim[^1]\n\n[^1]: the source",
      &ScopeName::new("scope".to_string()).unwrap(),
      &PackageName::new("pkg".to_string()).unwrap(),
      &Version::new("1.0.0").unwrap(),
      None,
      "/README.md",
    )
    .unwrap();

    // the footnote section wrapper and both link classes survive the allowlist
    assert!(html.contains(r#"<section class="footnotes">"#), "{html}");
    assert!(html.contains(r#"<sup class="footnote-ref">"#), "{html}");
    assert!(html.contains(r#"class="footnote-backref""#), "{html}");
    // and so do the anchors that make the jump there and back work
    assert!(html.contains(r##"href="#fn-1""##), "{html}");
    assert!(html.contains(r#"id="fn-1""#), "{html}");
    assert!(html.contains(r##"href="#fnref-1""##), "{html}");
    assert!(html.contains(r#"id="fnref-1""#), "{html}");
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

    // Root-relative links resolve against the package root (see #768).
    assert_eq!(rewriter(None, "/LICENSE"), "/@foo/bar/1.2.3/LICENSE");

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

    // Regression for #768: root-relative links resolve against the repository
    // root rather than being left to 404 against jsr.io.
    assert_eq!(
      rewriter(None, "/LICENSE"),
      "https://github.com/foo/bar/blob/HEAD/LICENSE"
    );
    assert_eq!(
      rewriter(None, "/assets/logo.svg"),
      "https://raw.githubusercontent.com/foo/bar/HEAD/assets/logo.svg"
    );
    // Protocol-relative URLs are left untouched.
    assert_eq!(rewriter(None, "//example.com/x"), "//example.com/x");
  }
}
