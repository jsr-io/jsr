// Copyright 2024 the JSR authors. All rights reserved. MIT license.
use std::collections::HashMap;
use std::sync::Arc;

use deno_ast::ModuleSpecifier;
use deno_error::JsErrorBox;
use deno_graph::BuildFastCheckTypeGraphOptions;
use deno_graph::BuildOptions;
use deno_graph::GraphKind;
use deno_graph::ModuleGraph;
use deno_graph::WorkspaceFastCheckOption;
use deno_graph::WorkspaceMember;
use deno_graph::ast::CapturingModuleAnalyzer;
use deno_graph::source::JsrUrlProvider;
use deno_graph::source::LoadError;
use deno_graph::source::LoadOptions;
use deno_graph::source::NullFileSystem;
use deno_graph::source::load_data_url;
use deno_semver::StackString;
use deno_semver::Version;
use deno_semver::jsr::JsrPackageReqReference;
use deno_semver::package::PackageNv;
use futures::FutureExt;
use indexmap::IndexMap;
use url::Url;

pub struct PassthroughJsrUrlProvider;

impl JsrUrlProvider for PassthroughJsrUrlProvider {
  fn url(&self) -> &Url {
    unreachable!(
      "BuildOptions::passthrough_jsr_specifiers should be set to true"
    )
  }

  fn package_url(&self, _nv: &PackageNv) -> Url {
    unreachable!(
      "BuildOptions::passthrough_jsr_specifiers should be set to true"
    )
  }

  fn package_url_to_nv(&self, _url: &Url) -> Option<PackageNv> {
    None
  }
}

#[derive(Debug)]
pub struct JsrResolver {
  pub member: WorkspaceMember,
}

impl deno_graph::source::Resolver for JsrResolver {
  fn resolve(
    &self,
    specifier_text: &str,
    referrer_range: &deno_graph::Range,
    _kind: deno_graph::source::ResolutionKind,
  ) -> Result<ModuleSpecifier, deno_graph::source::ResolveError> {
    if let Ok(package_ref) = JsrPackageReqReference::from_str(specifier_text)
      && self.member.name == package_ref.req().name
      && self
        .member
        .version
        .as_ref()
        .map(|v| package_ref.req().version_req.matches(v))
        .unwrap_or(true)
    {
      let export_name = package_ref.sub_path().unwrap_or(".");
      let Some(export) = self.member.exports.get(export_name) else {
        return Err(deno_graph::source::ResolveError::Other(
          JsErrorBox::generic(format!(
            "export '{}' not found in jsr:{}",
            export_name, self.member.name
          )),
        ));
      };
      return Ok(self.member.base.join(export).unwrap());
    }

    Ok(deno_graph::resolve_import(
      specifier_text,
      &referrer_range.specifier,
    )?)
  }
}

/// The module graph of a package, built the way the registry builds it.
pub struct PackageGraph {
  pub graph: ModuleGraph,
  pub analyzer: CapturingModuleAnalyzer,
}

/// Builds the module graph of a package from the files of its publish tarball
/// (keyed by their path in the package, like `/mod.ts`), with the fast check
/// type graph and its declarations.
pub async fn build_package_graph(
  name: &str,
  version: &Version,
  exports: &IndexMap<String, String>,
  files: &HashMap<String, Vec<u8>>,
) -> Result<PackageGraph, anyhow::Error> {
  let mut roots = vec![];
  for (key, path) in exports {
    let path = path.trim_start_matches('.').trim_start_matches('/');
    if !files.contains_key(&format!("/{path}")) {
      anyhow::bail!(
        "export '{key}' references entrypoint '/{path}' which does not exist"
      );
    }
    roots.push(Url::parse(&format!("file:///{path}"))?);
  }

  let workspace_member = WorkspaceMember {
    base: Url::parse("file:///").unwrap(),
    name: StackString::from_str(name),
    version: Some(version.clone()),
    exports: exports.clone(),
  };
  let workspace_members = vec![workspace_member.clone()];
  let analyzer = CapturingModuleAnalyzer::default();
  let mut graph = ModuleGraph::new(GraphKind::All);
  graph
    .build(
      roots,
      vec![],
      &MemoryLoader { files },
      BuildOptions {
        module_analyzer: &analyzer,
        file_system: &NullFileSystem,
        jsr_url_provider: &PassthroughJsrUrlProvider,
        passthrough_jsr_specifiers: true,
        resolver: Some(&JsrResolver {
          member: workspace_member,
        }),
        ..Default::default()
      },
    )
    .await;
  graph.valid()?;
  graph.build_fast_check_type_graph(BuildFastCheckTypeGraphOptions {
    fast_check_cache: None,
    fast_check_dts: true,
    jsr_url_provider: &PassthroughJsrUrlProvider,
    es_parser: Some(&analyzer),
    resolver: None,
    workspace_fast_check: WorkspaceFastCheckOption::Enabled(&workspace_members),
  });

  Ok(PackageGraph { graph, analyzer })
}

struct MemoryLoader<'a> {
  files: &'a HashMap<String, Vec<u8>>,
}

impl MemoryLoader<'_> {
  fn load_sync(
    &self,
    specifier: &ModuleSpecifier,
  ) -> deno_graph::source::LoadResult {
    match specifier.scheme() {
      "file" => {
        let Some(bytes) = self.files.get(specifier.path()).cloned() else {
          return Ok(None);
        };
        Ok(Some(deno_graph::source::LoadResponse::Module {
          content: bytes.into(),
          mtime: None,
          specifier: specifier.clone(),
          maybe_headers: None,
        }))
      }
      "http" | "https" | "node" | "npm" | "jsr" | "bun" | "virtual"
      | "cloudflare" => Ok(Some(deno_graph::source::LoadResponse::External {
        specifier: specifier.clone(),
      })),
      "data" => load_data_url(specifier)
        .map_err(|e| LoadError::Other(Arc::new(JsErrorBox::from_err(e)))),
      _ => Ok(None),
    }
  }
}

impl deno_graph::source::Loader for MemoryLoader<'_> {
  fn load(
    &self,
    specifier: &ModuleSpecifier,
    _options: LoadOptions,
  ) -> deno_graph::source::LoadFuture {
    let result = self.load_sync(specifier);
    async move { result }.boxed()
  }
}
