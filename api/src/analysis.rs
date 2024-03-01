// Copyright 2024 the JSR authors. All rights reserved. MIT license.
use std::cell::RefCell;
use std::collections::HashMap;
use std::collections::HashSet;
use std::sync::Arc;

use deno_ast::LineAndColumnDisplay;
use deno_ast::MediaType;
use deno_ast::ParsedSource;
use deno_ast::SourceRange;
use deno_ast::SourceRangedForSpanned;
use deno_doc::DocNodeKind;
use deno_graph::source::load_data_url;
use deno_graph::source::LoadOptions;
use deno_graph::source::NullFileSystem;
use deno_graph::BuildFastCheckTypeGraphOptions;
use deno_graph::BuildOptions;
use deno_graph::CapturingModuleAnalyzer;
use deno_graph::GraphKind;
use deno_graph::ModuleInfo;
use deno_graph::ModuleSpecifier;
use deno_graph::ParsedSourceStore;
use deno_graph::WorkspaceFastCheckOption;
use deno_graph::WorkspaceMember;
use deno_semver::jsr::JsrPackageReqReference;
use deno_semver::npm::NpmPackageReqReference;
use deno_semver::package::PackageNv;
use deno_semver::package::PackageReqReference;
use futures::FutureExt;
use once_cell::sync::Lazy;
use regex::Regex;
use tracing::debug;
use tracing::info_span;
use tracing::instrument;
use tracing::Instrument;
use url::Url;

use crate::buckets::BucketWithQueue;
use crate::db::DependencyKind;
use crate::db::ExportsMap;
use crate::db::PackageVersionMeta;
use crate::docs::DocNodesByUrl;
use crate::gcs_paths;
use crate::ids::PackageName;
use crate::ids::PackagePath;
use crate::ids::ScopeName;
use crate::ids::Version;
use crate::metadata::PackageMetadata;
use crate::metadata::VersionMetadata;
use crate::npm::create_npm_tarball;
use crate::npm::NpmTarball;
use crate::npm::NpmTarballOptions;
use crate::tarball::PublishError;

pub struct PackageAnalysisData {
  pub exports: ExportsMap,
  pub files: HashMap<PackagePath, Vec<u8>>,
}

pub struct PackageAnalysisOutput {
  pub data: PackageAnalysisData,
  pub module_graph_1: HashMap<String, ModuleInfo>,
  pub doc_nodes: DocNodesByUrl,
  pub dependencies: HashSet<(DependencyKind, PackageReqReference)>,
  pub npm_tarball: NpmTarball,
  pub readme_path: Option<PackagePath>,
  pub meta: PackageVersionMeta,
}

// We have to spawn another tokio runtime, because
// `deno_graph::ModuleGraph::build` is not thread-safe.
#[tokio::main(flavor = "current_thread")]
pub async fn analyze_package(
  span: tracing::Span,
  registry: Arc<dyn RegistryLoader>,
  scope: ScopeName,
  name: PackageName,
  version: Version,
  config_file: PackagePath,
  data: PackageAnalysisData,
) -> Result<PackageAnalysisOutput, PublishError> {
  analyze_package_inner(registry, scope, name, version, config_file, data)
    .instrument(span)
    .await
}

#[instrument(name = "analyze_package", skip(registry, data), err)]
async fn analyze_package_inner(
  registry: Arc<dyn RegistryLoader>,
  scope: ScopeName,
  name: PackageName,
  version: Version,
  config_file: PackagePath,
  data: PackageAnalysisData,
) -> Result<PackageAnalysisOutput, PublishError> {
  let PackageAnalysisData { exports, files } = data;
  let mut roots = vec![];
  let mut main_entrypoint = None;

  for (key, path) in exports.iter() {
    // Path is a relative path (./foo) to the config file.
    // This is always at the root, so it's also relative to the root of the tarball.
    let path = path.strip_prefix('.').unwrap();
    let path = PackagePath::new(path.to_string()).map_err(|error| {
      PublishError::InvalidPath {
        path: path.to_string(),
        error,
      }
    })?;
    if !files.contains_key(&path) {
      return Err(PublishError::ConfigFileExportsInvalid {
        path: Box::new(config_file.clone()),
        invalid_exports: format!(
          "export '{key}' references entrypoint '{path}' which does not exist",
        ),
      });
    }
    let url = Url::parse(&format!("file://{}", path)).unwrap();

    if key == "." {
      main_entrypoint = Some(url.clone());
    }

    roots.push(url);
  }

  let module_analyzer = ModuleAnalyzer::default();

  let workspace_members = vec![WorkspaceMember {
    base: Url::parse("file:///").unwrap(),
    exports: exports.clone().into_inner(),
    nv: PackageNv {
      name: format!("@{}/{}", scope, name),
      version: version.0.clone(),
    },
  }];
  let url_provider = RegistryJsrUrlProvider {
    registry: registry.as_ref(),
  };
  let mut graph = deno_graph::ModuleGraph::new(GraphKind::All);
  let diagnostics = graph
    .build(
      roots.clone(),
      &mut SyncLoader {
        files: &files,
        registry: registry.clone(),
      },
      BuildOptions {
        is_dynamic: false,
        module_analyzer: Some(&module_analyzer),
        workspace_members: &workspace_members,
        imports: Default::default(),
        // todo: use the data in the package for the file system
        file_system: Some(&NullFileSystem),
        jsr_url_provider: Some(&url_provider),
        resolver: None,
        npm_resolver: None,
        module_parser: Some(&module_analyzer.analyzer),
        reporter: None,
        executor: Default::default(),
      },
    )
    .await;
  assert!(diagnostics.is_empty());
  graph.valid().map_err(PublishError::GraphError)?;
  graph.build_fast_check_type_graph(BuildFastCheckTypeGraphOptions {
    fast_check_cache: None,
    fast_check_dts: true,
    jsr_url_provider: Some(&url_provider),
    module_parser: Some(&module_analyzer.analyzer),
    resolver: Default::default(),
    npm_resolver: Default::default(),
    workspace_fast_check: WorkspaceFastCheckOption::Enabled(&workspace_members),
  });

  for module in graph.modules() {
    // Check for global type augementation.
    // TODO(ry): this function should iterate through and returned back a
    // collection of errors instead of just the first one. That way we can say
    // everything wrong in one shot instead of the user fixing one error at a
    // time with each publish.
    if let Some(parsed_source) = module_analyzer
      .analyzer
      .get_parsed_source(module.specifier())
    {
      check_for_banned_syntax(&parsed_source)?;
      check_for_banned_triple_slash_directives(&parsed_source)?;
    }

    // Check that all modules are valid.
    match module.specifier().scheme() {
      "file" | "data" | "npm" | "node" => {}
      "http" | "https" => {
        if !module
          .specifier()
          .as_str()
          .starts_with(registry.registry_url().as_str())
        {
          return Err(PublishError::InvalidExternalImport {
            specifier: module.specifier().to_string(),
            info: "non-JSR http(s) import".to_string(),
          });
        }
      }
      _ => {
        return Err(PublishError::InvalidExternalImport {
          specifier: module.specifier().to_string(),
          info: "unsupported scheme".to_string(),
        });
      }
    }
  }

  let all_fast_check = graph
    .modules()
    .filter_map(|module| {
      if roots.contains(module.specifier()) {
        module.js()
      } else {
        None
      }
    })
    .all(|js| js.fast_check_module().is_some());

  let doc_nodes =
    crate::docs::generate_docs(roots, &graph, &module_analyzer.analyzer)
      .map_err(PublishError::DocError)?;

  let module_graph_1 = module_analyzer.take_module_graph_1();

  let dependencies = analyze_dependencies(&module_graph_1)?;

  let npm_tarball = create_npm_tarball(NpmTarballOptions {
    graph: &graph,
    sources: &module_analyzer.analyzer,
    registry_url: registry.registry_url(),
    scope: &scope,
    package: &name,
    version: &version,
    exports: &exports,
    dependencies: dependencies.iter(),
  })
  .map_err(PublishError::NpmTarballError)?;

  let (meta, readme_path) = {
    let readme = files.iter().find(|file| file.0.is_readme());

    (
      generate_score(main_entrypoint, &doc_nodes, &readme, all_fast_check),
      readme.map(|readme| readme.0.clone()),
    )
  };

  Ok(PackageAnalysisOutput {
    data: PackageAnalysisData { exports, files },
    module_graph_1,
    doc_nodes,
    dependencies,
    npm_tarball,
    readme_path,
    meta,
  })
}

fn generate_score(
  main_entrypoint: Option<ModuleSpecifier>,
  doc_nodes_by_url: &DocNodesByUrl,
  readme: &Option<(&PackagePath, &Vec<u8>)>,
  all_fast_check: bool,
) -> PackageVersionMeta {
  let main_entrypoint_doc =
    main_entrypoint.as_ref().and_then(|main_entrypoint| {
      doc_nodes_by_url
        .get(main_entrypoint)
        .unwrap()
        .iter()
        .find(|node| node.kind == DocNodeKind::ModuleDoc)
        .map(|node| &node.js_doc)
    });

  let has_readme_examples = readme
    .is_some_and(|(_, readme)| readme.windows(3).any(|chars| chars == b"```"))
    || main_entrypoint_doc.is_some_and(|js_doc| {
      js_doc.doc.as_ref().is_some_and(|doc| doc.contains("```"))
        || js_doc
          .tags
          .iter()
          .any(|tag| matches!(tag, deno_doc::js_doc::JsDocTag::Example { .. }))
    });

  PackageVersionMeta {
    has_readme: readme.is_some()
      || main_entrypoint_doc
        .is_some_and(|doc| doc.doc.as_ref().is_some_and(|doc| !doc.is_empty())),
    has_readme_examples,
    all_entrypoints_docs: all_entrypoints_have_module_doc(
      doc_nodes_by_url,
      main_entrypoint,
      readme.is_some(),
    ),
    percentage_documented_symbols: percentage_of_symbols_with_docs(
      doc_nodes_by_url,
    ),
    all_fast_check,
  }
}

fn all_entrypoints_have_module_doc(
  doc_nodes_by_url: &DocNodesByUrl,
  main_entrypoint: Option<ModuleSpecifier>,
  has_readme: bool,
) -> bool {
  'modules: for (specifier, nodes) in doc_nodes_by_url {
    for node in nodes {
      if node.kind == DocNodeKind::ModuleDoc {
        continue 'modules;
      }
    }

    if main_entrypoint
      .as_ref()
      .is_some_and(|main_entrypoint| main_entrypoint == specifier)
      && has_readme
    {
      continue 'modules;
    }

    return false;
  }

  true
}

fn percentage_of_symbols_with_docs(doc_nodes_by_url: &DocNodesByUrl) -> f32 {
  let mut total_symbols = 0;
  let mut documented_symbols = 0;

  for (_specifier, nodes) in doc_nodes_by_url {
    for node in nodes {
      if node.kind == DocNodeKind::ModuleDoc
        || node.kind == DocNodeKind::Import
        || node.declaration_kind == deno_doc::node::DeclarationKind::Private
      {
        continue;
      }

      total_symbols += 1;

      if !node.js_doc.is_empty() {
        documented_symbols += 1;
      }
    }
  }

  if total_symbols == 0 {
    return 1.0;
  }

  (documented_symbols as f32) / (total_symbols as f32)
}

fn analyze_dependencies(
  module_graph_1: &HashMap<String, ModuleInfo>,
) -> Result<HashSet<(DependencyKind, PackageReqReference)>, PublishError> {
  let mut dependencies = HashSet::new();
  for info in module_graph_1.values() {
    for dep in &info.dependencies {
      let Some(d) = dep.as_static() else {
        continue;
      };
      if d.specifier.starts_with("jsr:") {
        let req = JsrPackageReqReference::from_str(&d.specifier)
          .map_err(PublishError::InvalidJsrSpecifier)?;

        if req.req().version_req.version_text() == "*" {
          return Err(PublishError::JsrMissingConstraint(req));
        }
        dependencies.insert((DependencyKind::Jsr, req.into_inner()));
      }
      if d.specifier.starts_with("npm:") {
        let req = NpmPackageReqReference::from_str(&d.specifier)
          .map_err(PublishError::InvalidNpmSpecifier)?;

        if req.req().version_req.version_text() == "*" {
          return Err(PublishError::NpmMissingConstraint(req));
        }
        dependencies.insert((DependencyKind::Npm, req.into_inner()));
      }
    }
  }
  Ok(dependencies)
}

struct RegistryJsrUrlProvider<'a> {
  registry: &'a dyn RegistryLoader,
}

impl<'a> deno_graph::source::JsrUrlProvider for RegistryJsrUrlProvider<'a> {
  fn url(&self) -> &Url {
    self.registry.registry_url()
  }
}

struct SyncLoader<'a> {
  files: &'a HashMap<PackagePath, Vec<u8>>,
  registry: Arc<dyn RegistryLoader>,
}

impl<'a> SyncLoader<'a> {
  fn load_sync(
    &mut self,
    specifier: &ModuleSpecifier,
  ) -> deno_graph::source::LoadResult {
    match specifier.scheme() {
      "file" => {
        let Ok(path) = PackagePath::new(specifier.path().to_string()) else {
          return Ok(None);
        };
        let Some(bytes) = self.files.get(&path).cloned() else {
          return Ok(None);
        };
        Ok(Some(deno_graph::source::LoadResponse::Module {
          content: bytes.into(),
          specifier: specifier.clone(),
          maybe_headers: None,
        }))
      }
      "http" | "https" | "node" | "npm" => {
        Ok(Some(deno_graph::source::LoadResponse::External {
          specifier: specifier.clone(),
        }))
      }
      "data" => load_data_url(specifier),
      _ => Ok(None),
    }
  }

  fn parse_registry_url(
    &self,
    specifier: &ModuleSpecifier,
  ) -> Option<(ScopeName, PackageName, Option<Version>)> {
    // todo(dsherret): deja-vu with `parse_registry_url` in `GcsLoader`.
    if let Some(path) = specifier
      .as_str()
      .strip_prefix(self.registry.registry_url().as_str())
    {
      let mut split = path.split('/').peekable();
      if split.peek() == Some(&"") {
        split.next();
      }
      let scope =
        ScopeName::new(split.next()?.strip_prefix('@')?.into()).ok()?;
      let name = PackageName::new(split.next()?.into()).ok()?;
      let path = split.next()?;
      if split.next().is_some() {
        return None;
      }
      if path == "meta.json" {
        return Some((scope, name, None));
      }
      if let Some(version) = path.strip_suffix("_meta.json") {
        let version = Version::new(version).ok()?;
        return Some((scope, name, Some(version)));
      }
    }
    None
  }
}

impl<'a> deno_graph::source::Loader for SyncLoader<'a> {
  fn load(
    &mut self,
    specifier: &ModuleSpecifier,
    _options: LoadOptions,
  ) -> deno_graph::source::LoadFuture {
    if let Some((scope, name, version)) = self.parse_registry_url(specifier) {
      let specifier = specifier.clone();
      let registry = self.registry.clone();
      let span = info_span!("Loader::load", specifier = %specifier);
      async move {
        if let Some(version) = version {
          debug!("Fetching package version manifest from registry: {scope}/{name}@{version}");
          let version_meta = registry
            .load_version_meta(&scope, &name, &version)
            .await?;
          let content = serde_json::to_vec(&version_meta)?;
          Ok(Some(deno_graph::source::LoadResponse::Module {
            content: content.into(),
            specifier,
            maybe_headers: None,
          }))
        } else {
          debug!("Fetching package manifest from registry: {scope}/{name}");
          let Some(meta) = registry.load_package_meta(&scope, &name).await? else {
            return Ok(None);
          };
          let content = serde_json::to_vec(&meta)?;
          Ok(Some(deno_graph::source::LoadResponse::Module {
            content: content.into(),
            specifier,
            maybe_headers: None,
          }))
        }
      }
      .instrument(span)
      .boxed()
    } else {
      let result = self.load_sync(specifier);
      async move { result }.boxed()
    }
  }
}

pub struct RebuildNpmTarballData {
  pub scope: ScopeName,
  pub name: PackageName,
  pub version: Version,
  pub exports: ExportsMap,
  pub files: HashSet<PackagePath>,
  pub dependencies: Vec<(DependencyKind, PackageReqReference)>,
}

// We have to spawn another tokio runtime, because
// `deno_graph::ModuleGraph::build` is not thread-safe.
#[tokio::main(flavor = "current_thread")]
pub async fn rebuild_npm_tarball(
  span: tracing::Span,
  registry: Arc<dyn RegistryLoader>,
  modules_bucket: BucketWithQueue,
  data: RebuildNpmTarballData,
) -> Result<NpmTarball, anyhow::Error> {
  rebuild_npm_tarball_inner(registry, modules_bucket, data)
    .instrument(span)
    .await
}

#[instrument(
  name = "rebuild_npm_tarball",
  skip(registry, modules_bucket, data),
  err
)]
async fn rebuild_npm_tarball_inner(
  registry: Arc<dyn RegistryLoader>,
  modules_bucket: BucketWithQueue,
  data: RebuildNpmTarballData,
) -> Result<NpmTarball, anyhow::Error> {
  let RebuildNpmTarballData {
    scope,
    name,
    version,
    exports,
    files,
    dependencies,
  } = data;

  let mut roots = vec![];
  for (_, path) in exports.iter() {
    // Path is a relative path (./foo) to config file. This is always at the root,
    // so it's also relative to the root of the tarball.
    let path = path.strip_prefix('.').unwrap();
    let path = PackagePath::new(path.to_string()).map_err(|error| {
      PublishError::InvalidPath {
        path: path.to_string(),
        error,
      }
    })?;
    let url = Url::parse(&format!("file://{}", path)).unwrap();
    roots.push(url);
  }

  let module_analyzer = ModuleAnalyzer::default();

  let mut graph = deno_graph::ModuleGraph::new(GraphKind::All);
  let workspace_members = vec![WorkspaceMember {
    base: Url::parse("file:///").unwrap(),
    exports: exports.clone().into_inner(),
    nv: PackageNv {
      name: format!("@{}/{}", scope, name),
      version: version.0.clone(),
    },
  }];
  let url_provider = RegistryJsrUrlProvider {
    registry: registry.as_ref(),
  };
  let diagnostics = graph
    .build(
      roots.clone(),
      &mut GcsLoader {
        files: &files,
        bucket: &modules_bucket,
        scope: &scope,
        name: &name,
        version: &version,
        registry: registry.clone(),
      },
      BuildOptions {
        is_dynamic: false,
        module_analyzer: Some(&module_analyzer),
        workspace_members: &workspace_members,
        imports: Default::default(),
        // todo: use the data in the package for the file system
        file_system: Some(&NullFileSystem),
        jsr_url_provider: Some(&url_provider),
        resolver: Default::default(),
        npm_resolver: Default::default(),
        module_parser: Some(&module_analyzer.analyzer),
        reporter: Default::default(),
        executor: Default::default(),
      },
    )
    .await;
  assert!(diagnostics.is_empty());
  graph.valid()?;
  graph.build_fast_check_type_graph(BuildFastCheckTypeGraphOptions {
    fast_check_cache: Default::default(),
    fast_check_dts: true,
    jsr_url_provider: Some(&url_provider),
    module_parser: Some(&module_analyzer.analyzer),
    resolver: None,
    npm_resolver: None,
    workspace_fast_check: WorkspaceFastCheckOption::Enabled(&workspace_members),
  });

  let npm_tarball = create_npm_tarball(NpmTarballOptions {
    graph: &graph,
    sources: &module_analyzer.analyzer,
    registry_url: registry.registry_url(),
    scope: &scope,
    package: &name,
    version: &version,
    exports: &exports,
    dependencies: dependencies.iter(),
  })?;

  Ok(npm_tarball)
}

struct GcsLoader<'a> {
  files: &'a HashSet<PackagePath>,
  bucket: &'a BucketWithQueue,
  scope: &'a ScopeName,
  name: &'a PackageName,
  version: &'a Version,
  registry: Arc<dyn RegistryLoader>,
}

impl<'a> GcsLoader<'a> {
  fn load_inner(
    &mut self,
    specifier: &ModuleSpecifier,
  ) -> deno_graph::source::LoadFuture {
    let specifier = specifier.clone();
    match specifier.scheme() {
      "file" => {
        let Ok(path) = PackagePath::new(specifier.path().to_string()) else {
          return async move { Ok(None) }.boxed();
        };
        if !self.files.contains(&path) {
          return async move { Ok(None) }.boxed();
        };
        let gcs_path =
          gcs_paths::file_path(self.scope, self.name, self.version, &path);
        let bucket = self.bucket.clone();
        async move {
          let Some(bytes) = bucket.download(gcs_path.into()).await? else {
            return Ok(None);
          };
          Ok(Some(deno_graph::source::LoadResponse::Module {
            content: bytes.to_vec().into(),
            specifier,
            maybe_headers: None,
          }))
        }
        .boxed()
      }
      "http" | "https" | "node" | "npm" => async move {
        Ok(Some(deno_graph::source::LoadResponse::External {
          specifier,
        }))
      }
      .boxed(),
      "data" => async move { load_data_url(&specifier) }.boxed(),
      _ => async move { Ok(None) }.boxed(),
    }
  }

  fn parse_registry_url(
    &self,
    specifier: &ModuleSpecifier,
  ) -> Option<(ScopeName, PackageName, Option<Version>)> {
    if let Some(path) = specifier
      .as_str()
      .strip_prefix(self.registry.registry_url().as_str())
    {
      let mut split = path.split('/').peekable();
      if split.peek() == Some(&"") {
        split.next();
      }
      let scope =
        ScopeName::new(split.next()?.strip_prefix('@')?.into()).ok()?;
      let name = PackageName::new(split.next()?.into()).ok()?;
      let path = split.next()?;
      if split.next().is_some() {
        return None;
      }
      if path == "meta.json" {
        return Some((scope, name, None));
      }
      if let Some(version) = path.strip_suffix("_meta.json") {
        let version = Version::new(version).ok()?;
        return Some((scope, name, Some(version)));
      }
    }
    None
  }
}

impl<'a> deno_graph::source::Loader for GcsLoader<'a> {
  fn load(
    &mut self,
    specifier: &ModuleSpecifier,
    _options: LoadOptions,
  ) -> deno_graph::source::LoadFuture {
    if let Some((scope, name, version)) = self.parse_registry_url(specifier) {
      let specifier = specifier.clone();
      let registry = self.registry.clone();
      let span = info_span!("Loader::load", specifier = %specifier);
      async move {
        if let Some(version) = version {
          debug!("Fetching package version manifest from registry: {scope}/{name}@{version}");
          let version_meta = registry
            .load_version_meta(&scope, &name, &version)
            .await?;
          let content = serde_json::to_vec(&version_meta)?;
          Ok(Some(deno_graph::source::LoadResponse::Module {
            content: content.into(),
            specifier,
            maybe_headers: None,
          }))
        } else {
          debug!("Fetching package manifest from registry: {scope}/{name}");
          let Some(meta) = registry.load_package_meta(&scope, &name).await? else {
            return Ok(None);
          };
          let content = serde_json::to_vec(&meta)?;
          Ok(Some(deno_graph::source::LoadResponse::Module {
            content: content.into(),
            specifier,
            maybe_headers: None,
          }))
        }
      }
      .instrument(span)
      .boxed()
    } else {
      self.load_inner(specifier)
    }
  }
}

#[derive(Default)]
struct ModuleAnalyzer {
  pub analyzer: CapturingModuleAnalyzer,
  pub module_info: RefCell<HashMap<Url, ModuleInfo>>,
}

impl ModuleAnalyzer {
  fn take_module_graph_1(&self) -> HashMap<String, ModuleInfo> {
    std::mem::take(&mut *self.module_info.borrow_mut())
      .into_iter()
      .filter_map(|(url, info)| {
        if url.scheme() == "file" {
          let path = url.path();
          Some((path.to_string(), info))
        } else {
          None
        }
      })
      .collect()
  }
}

impl deno_graph::ModuleAnalyzer for ModuleAnalyzer {
  fn analyze(
    &self,
    specifier: &deno_ast::ModuleSpecifier,
    source: Arc<str>,
    media_type: MediaType,
  ) -> Result<ModuleInfo, deno_ast::ParseDiagnostic> {
    let module_info = self.analyzer.analyze(specifier, source, media_type)?;
    self
      .module_info
      .borrow_mut()
      .insert(specifier.clone(), module_info.clone());
    Ok(module_info)
  }
}

#[async_trait::async_trait]
pub trait RegistryLoader: Send + Sync + 'static {
  fn registry_url(&self) -> &Url;

  async fn load_package_meta(
    &self,
    scope: &ScopeName,
    name: &PackageName,
  ) -> Result<Option<PackageMetadata>, anyhow::Error>;

  async fn load_version_meta(
    &self,
    scope: &ScopeName,
    name: &PackageName,
    version: &Version,
  ) -> Result<VersionMetadata, anyhow::Error>;
}

pub struct GcsRegistryLoader {
  registry_url: Url,
  module_bucket: BucketWithQueue,
}

impl GcsRegistryLoader {
  pub fn new(registry_url: Url, module_bucket: BucketWithQueue) -> Self {
    Self {
      registry_url,
      module_bucket,
    }
  }
}

#[async_trait::async_trait]
impl RegistryLoader for GcsRegistryLoader {
  fn registry_url(&self) -> &Url {
    &self.registry_url
  }

  #[tracing::instrument(
    name = "GcsRegistryLoader::load_package_meta",
    skip(self),
    err
  )]
  async fn load_package_meta(
    &self,
    scope: &ScopeName,
    name: &PackageName,
  ) -> Result<Option<PackageMetadata>, anyhow::Error> {
    let path = format!("@{}/{}/meta.json", scope, name);
    let Some(bytes) = self.module_bucket.download(path.into()).await? else {
      return Ok(None);
    };
    let meta = serde_json::from_slice(&bytes)?;
    Ok(Some(meta))
  }

  #[tracing::instrument(
    name = "GcsRegistryLoader::load_version_meta",
    skip(self),
    err
  )]
  async fn load_version_meta(
    &self,
    scope: &ScopeName,
    name: &PackageName,
    version: &Version,
  ) -> Result<VersionMetadata, anyhow::Error> {
    let path: Arc<str> =
      format!("@{}/{}/{}_meta.json", scope, name, version).into();
    let bytes = self
      .module_bucket
      .download(path.clone())
      .await?
      .ok_or_else(|| {
        anyhow::anyhow!("failed to find version meta at {}", path)
      })?;
    let meta = serde_json::from_slice(&bytes)?;
    Ok(meta)
  }
}

fn check_for_banned_syntax(
  parsed_source: &ParsedSource,
) -> Result<(), PublishError> {
  use deno_ast::swc::ast;

  let line_col = |range: &SourceRange| -> (usize, usize) {
    let LineAndColumnDisplay {
      line_number,
      column_number,
    } = parsed_source
      .text_info()
      .line_and_column_display(range.start);
    (line_number, column_number)
  };

  for i in parsed_source.module().body.iter() {
    match i {
      ast::ModuleItem::ModuleDecl(n) => match n {
        ast::ModuleDecl::TsNamespaceExport(n) => {
          let (line, column) = line_col(&n.range());
          return Err(PublishError::GlobalTypeAugmentation {
            specifier: parsed_source.specifier().to_string(),
            line,
            column,
          });
        }
        ast::ModuleDecl::TsExportAssignment(n) => {
          let (line, column) = line_col(&n.range());
          return Err(PublishError::GlobalTypeAugmentation {
            specifier: parsed_source.specifier().to_string(),
            line,
            column,
          });
        }
        ast::ModuleDecl::TsImportEquals(n) => match n.module_ref {
          ast::TsModuleRef::TsExternalModuleRef(_) => {
            let (line, column) = line_col(&n.range());
            return Err(PublishError::CommonJs {
              specifier: parsed_source.specifier().to_string(),
              line,
              column,
            });
          }
          _ => {
            continue;
          }
        },
        _ => continue,
      },
      ast::ModuleItem::Stmt(n) => match n {
        ast::Stmt::Decl(ast::Decl::TsModule(n)) => {
          if n.global {
            let (line, column) = line_col(&n.range());
            return Err(PublishError::GlobalTypeAugmentation {
              specifier: parsed_source.specifier().to_string(),
              line,
              column,
            });
          }
          match &n.id {
            ast::TsModuleName::Str(n) => {
              let (line, column) = line_col(&n.range());
              return Err(PublishError::GlobalTypeAugmentation {
                specifier: parsed_source.specifier().to_string(),
                line,
                column,
              });
            }
            _ => continue,
          }
        }
        _ => continue,
      },
    }
  }
  Ok(())
}

static TRIPLE_SLASH_RE: Lazy<Regex> = Lazy::new(|| {
  Regex::new(
    r#"/\s+<reference\s+(no-default-lib\s*=\s*"true"|lib\s*=\s*("[^"]+"|'[^']+'))\s*/>"#,
  )
  .unwrap()
});

fn check_for_banned_triple_slash_directives(
  parsed_source: &ParsedSource,
) -> Result<(), PublishError> {
  let comments = parsed_source.comments().leading_map();
  for (_pos, comments) in comments.iter() {
    for comment in comments {
      if TRIPLE_SLASH_RE.is_match(&comment.text) {
        let lc = parsed_source
          .text_info()
          .line_and_column_display(comment.range().start);
        return Err(PublishError::BannedTripleSlashDirectives {
          specifier: parsed_source.specifier().to_string(),
          line: lc.line_number,
          column: lc.column_number,
        });
      }
    }
  }
  Ok(())
}

#[cfg(test)]
mod tests {
  fn parse(source: &str) -> deno_ast::ParsedSource {
    let specifier = deno_ast::ModuleSpecifier::parse("file:///mod.ts").unwrap();
    let media_type = deno_ast::MediaType::TypeScript;
    deno_ast::parse_module(deno_ast::ParseParams {
      specifier,
      text_info: deno_ast::SourceTextInfo::new(source.into()),
      media_type,
      capture_tokens: false,
      scope_analysis: false,
      maybe_syntax: None,
    })
    .unwrap()
  }

  #[test]
  fn banned_triple_slash_directives() {
    let x = parse("let x = 1;");
    assert!(super::check_for_banned_triple_slash_directives(&x).is_ok());

    let x = parse("/// <reference lib=\"dom\" />");
    let err = super::check_for_banned_triple_slash_directives(&x).unwrap_err();
    assert!(
      matches!(err, super::PublishError::BannedTripleSlashDirectives { .. }),
      "{err:?}",
    );

    let x = parse("/// <reference no-default-lib=\"true\" />");
    let err = super::check_for_banned_triple_slash_directives(&x).unwrap_err();
    assert!(
      matches!(err, super::PublishError::BannedTripleSlashDirectives { .. }),
      "{err:?}",
    );

    let x = parse("///   <reference   no-default-lib=\"true\"/>");
    let err = super::check_for_banned_triple_slash_directives(&x).unwrap_err();
    assert!(
      matches!(err, super::PublishError::BannedTripleSlashDirectives { .. }),
      "{err:?}",
    );

    let x = parse("///   <reference   no-default-lib = \"true\"/>");
    let err = super::check_for_banned_triple_slash_directives(&x).unwrap_err();
    assert!(
      matches!(err, super::PublishError::BannedTripleSlashDirectives { .. }),
      "{err:?}",
    );

    let x = parse("    /// <reference   lib = \"dom\"/>");
    let err = super::check_for_banned_triple_slash_directives(&x).unwrap_err();
    assert!(
      matches!(err, super::PublishError::BannedTripleSlashDirectives { .. }),
      "{err:?}",
    );

    let x = parse("   ///   <reference   lib = \'dom\'/>");
    let err = super::check_for_banned_triple_slash_directives(&x).unwrap_err();
    assert!(
      matches!(err, super::PublishError::BannedTripleSlashDirectives { .. }),
      "{err:?}",
    );
  }

  #[test]
  fn banned_syntax() {
    let x = parse("let x = 1;");
    assert!(super::check_for_banned_syntax(&x).is_ok());

    let x = parse("global {}");
    let err = super::check_for_banned_syntax(&x).unwrap_err();
    assert!(
      matches!(err, super::PublishError::GlobalTypeAugmentation { .. }),
      "{err:?}",
    );

    let x = parse("let x = 1; global {}");
    let err = super::check_for_banned_syntax(&x).unwrap_err();
    assert!(
      matches!(err, super::PublishError::GlobalTypeAugmentation { .. }),
      "{err:?}",
    );

    let x = parse("declare module foo { }");
    assert!(super::check_for_banned_syntax(&x).is_ok());

    let x = parse("declare module \"x\" { }");
    let err = super::check_for_banned_syntax(&x).unwrap_err();
    assert!(
      matches!(err, super::PublishError::GlobalTypeAugmentation { .. }),
      "{err:?}",
    );

    let x = parse("import foo from \"foo\"");
    assert!(super::check_for_banned_syntax(&x).is_ok());

    let x = parse("export as namespace React;");
    let err = super::check_for_banned_syntax(&x).unwrap_err();
    assert!(
      matches!(err, super::PublishError::GlobalTypeAugmentation { .. }),
      "{err:?}",
    );

    let x = parse("export = {}");
    let err = super::check_for_banned_syntax(&x).unwrap_err();
    assert!(
      matches!(err, super::PublishError::GlobalTypeAugmentation { .. }),
      "{err:?}",
    );

    let x = parse("import express = require('foo');");
    let err = super::check_for_banned_syntax(&x).unwrap_err();
    assert!(
      matches!(err, super::PublishError::CommonJs { .. }),
      "{err:?}",
    );

    let x = parse("import express = React.foo;");
    assert!(super::check_for_banned_syntax(&x).is_ok());
  }
}
