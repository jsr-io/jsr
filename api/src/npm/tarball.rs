// Copyright 2024 the JSR authors. All rights reserved. MIT license.
use std::borrow::Cow;

use anyhow::Context;
use base64::Engine;
use deno_graph::ModuleGraph;
use deno_graph::ParsedSourceStore;
use deno_semver::package::PackageReqReference;
use indexmap::IndexMap;
use sha2::Digest;
use tar::Header;
use tracing::error;
use tracing::info;
use url::Url;

use crate::db::DependencyKind;
use crate::db::ExportsMap;
use crate::ids::PackageName;
use crate::ids::ScopeName;
use crate::ids::ScopedPackageName;
use crate::ids::Version;
use crate::npm::specifiers::rewrite_extension;
use crate::npm::specifiers::rewrite_specifier;
use crate::npm::specifiers::Extension;
use crate::npm::types::NpmMappedJsrPackageName;
use crate::npm::types::NpmPackageJson;

use super::emit::transpile_to_js;

pub struct NpmTarball {
  /// The gzipped tarball contents.
  pub tarball: Vec<u8>,
  /// The hex encoded sha1 hash of the gzipped tarball.
  pub sha1: String,
  /// The base64 encoded sha512 hash of the gzipped tarball.
  pub sha512: String,
}

pub struct NpmTarballOptions<
  'a,
  Deps: Iterator<Item = &'a (DependencyKind, PackageReqReference)>,
> {
  pub graph: &'a ModuleGraph,
  pub sources: &'a dyn ParsedSourceStore,
  pub registry_url: &'a Url,
  pub scope: &'a ScopeName,
  pub package: &'a PackageName,
  pub version: &'a Version,
  pub exports: &'a ExportsMap,
  pub dependencies: Deps,
}

pub fn create_npm_tarball<'a>(
  opts: NpmTarballOptions<
    'a,
    impl Iterator<Item = &'a (DependencyKind, PackageReqReference)>,
  >,
) -> Result<NpmTarball, anyhow::Error> {
  let NpmTarballOptions {
    graph,
    sources,
    registry_url,
    scope,
    package,
    version,
    exports,
    dependencies,
  } = opts;

  let npm_package_id = NpmMappedJsrPackageName { scope, package };

  let npm_exports = create_npm_exports(exports);

  let npm_dependencies =
    create_npm_dependencies(dependencies.map(Cow::Borrowed))?;

  let homepage = Url::options()
    .base_url(Some(registry_url))
    .parse(&format!("./@{scope}/{package}",))
    .unwrap()
    .to_string();

  let pkg_json = NpmPackageJson {
    name: npm_package_id,
    version: version.clone(),
    module_type: "module".to_string(),
    exports: npm_exports,
    dependencies: npm_dependencies,
    homepage,
  };

  let mut transpiled_files = IndexMap::new();

  for module in graph.modules() {
    if module.specifier().scheme() != "file" {
      continue;
    };
    let path = module.specifier().path();

    if let Some(json) = module.json() {
      transpiled_files.insert(path.to_owned(), json.source.as_bytes().to_vec());
    } else if let Some(js) = module.js() {
      match js.media_type {
        // We need to transpile js files too, to rewrite import sources
        // from `npm:*` to bare specifiers, for example.
        deno_ast::MediaType::JavaScript | deno_ast::MediaType::Mjs => {
          let source = sources
            .get_parsed_source(module.specifier())
            .expect("parsed source should be here");
          let source_url = Url::options()
            .base_url(Some(registry_url))
            .parse(&format!("./@{scope}/{package}/{version}{path}",))
            .unwrap();
          let transpiled = transpile_to_js(source, source_url)
            .with_context(|| format!("failed to transpile {}", path))?;

          transpiled_files
            .insert(path.to_owned(), transpiled.as_bytes().to_vec());
        }
        deno_ast::MediaType::Dts | deno_ast::MediaType::Dmts => {
          transpiled_files
            .insert(path.to_owned(), js.source.as_bytes().to_vec());
        }
        deno_ast::MediaType::Jsx
        | deno_ast::MediaType::TypeScript
        | deno_ast::MediaType::Mts
        | deno_ast::MediaType::Tsx => {
          let source = sources
            .get_parsed_source(module.specifier())
            .expect("parsed source should be here");
          let source_url = Url::options()
            .base_url(Some(registry_url))
            .parse(&format!("./@{scope}/{package}/{version}{path}",))
            .unwrap();
          let transpiled = transpile_to_js(source, source_url)
            .with_context(|| format!("failed to transpile {}", path))?;

          let rewritten_path = rewrite_extension(path, Extension::Js)
            .unwrap_or_else(|| path.to_owned());
          transpiled_files
            .insert(rewritten_path, transpiled.as_bytes().to_vec());
        }
        _ => {}
      }

      // Dts files
      if let Some(fsm) = js.fast_check_module() {
        if let Some(dts) = &fsm.dts {
          if !dts.diagnostics.is_empty() {
            let message = dts
              .diagnostics
              .iter()
              .map(|d| match d.range() {
                Some(range) => {
                  format!("{}, at {}@{}", d, range.specifier, range.range.start)
                }
                None => format!("{}, at {}", d, d.specifier()),
              })
              .collect::<Vec<_>>()
              .join(", ");
            info!(
              "Npm dts generation @{}/{}@{}: {}",
              scope, package, version, message
            );
          }

          let rewritten_path = rewrite_extension(path, Extension::Dts)
            .unwrap_or_else(|| path.to_owned());
          transpiled_files.insert(rewritten_path, dts.text.as_bytes().to_vec());
        }
      }
    }
  }

  let pkg_json_str = serde_json::to_string_pretty(&pkg_json)?;
  transpiled_files.insert("/package.json".to_string(), pkg_json_str.into());

  transpiled_files.sort_keys();

  let mut tar_gz_bytes = Vec::new();
  let mut gz_encoder = flate2::write::GzEncoder::new(
    &mut tar_gz_bytes,
    flate2::Compression::default(),
  );
  let mut tarball = tar::Builder::new(&mut gz_encoder);

  let now = std::time::SystemTime::now();
  let mtime = now.duration_since(std::time::UNIX_EPOCH).unwrap().as_secs();

  for (path, content) in transpiled_files.iter() {
    let mut header = Header::new_ustar();
    header.set_path(format!("./package{path}")).map_err(|e| {
      // Ideally we never hit this error, because package length should have been checked
      // when creating PackagePath.
      // TODO(ry) This is not the ideal way to pass PublishErrors up the stack
      // because it will become anyhow::Error and wrapped in an NpmTarballError.
      error!("bad npm tarball path {} {}", path, e);
      crate::tarball::PublishError::InvalidPath {
        path: path.to_string(),
        error: crate::ids::PackagePathValidationError::TooLong(path.len()),
      }
    })?;
    header.set_size(content.len() as u64);
    header.set_mode(0o777);
    header.set_mtime(mtime);
    header.set_cksum();
    tarball.append(&header, content.as_slice()).unwrap();
  }

  tarball.into_inner().unwrap();
  gz_encoder.finish().unwrap();

  let sha1_digest = sha1::Sha1::digest(&tar_gz_bytes);
  let sha1 = format!("{sha1_digest:X}");
  let sha512_digest = sha2::Sha512::digest(&tar_gz_bytes);
  let sha512 = base64::prelude::BASE64_STANDARD.encode(sha512_digest);

  Ok(NpmTarball {
    tarball: tar_gz_bytes,
    sha1,
    sha512,
  })
}

pub fn create_npm_dependencies<'a>(
  dependencies: impl Iterator<Item = Cow<'a, (DependencyKind, PackageReqReference)>>,
) -> Result<IndexMap<String, String>, anyhow::Error> {
  let mut npm_dependencies = IndexMap::new();
  for dep in dependencies {
    let (kind, req) = &*dep;
    match kind {
      DependencyKind::Jsr => {
        let jsr_name = ScopedPackageName::new(req.req.name.clone())?;
        let npm_name = NpmMappedJsrPackageName {
          scope: &jsr_name.scope,
          package: &jsr_name.package,
        };
        npm_dependencies
          .insert(npm_name.to_string(), req.req.version_req.to_string());
      }
      DependencyKind::Npm => {
        npm_dependencies
          .insert(req.req.name.clone(), req.req.version_req.to_string());
      }
    }
  }
  npm_dependencies.sort_keys();
  Ok(npm_dependencies)
}

pub fn create_npm_exports(exports: &ExportsMap) -> IndexMap<String, String> {
  let mut npm_exports = IndexMap::new();
  for (key, path) in exports.iter() {
    // TODO: insert types exports here also
    let import_path =
      rewrite_specifier(path).unwrap_or_else(|| path.to_owned());
    npm_exports.insert(key.clone(), import_path);
  }
  npm_exports
}

#[cfg(test)]
mod tests {
  use std::{collections::HashMap, io::Read};

  use async_tar::Archive;
  use deno_ast::ModuleSpecifier;
  use deno_graph::{
    source::{MemoryLoader, NullFileSystem, Source},
    BuildOptions, GraphKind, ModuleGraph, WorkspaceMember,
  };
  use deno_semver::package::{PackageNv, PackageReqReference};
  use futures::{AsyncReadExt, StreamExt};
  use indexmap::IndexMap;
  use url::Url;

  use crate::{
    analysis::ModuleAnalyzer,
    db::{DependencyKind, ExportsMap},
    ids::{PackageName, ScopeName, Version},
  };

  use super::{create_npm_tarball, NpmTarballOptions};

  async fn test_npm_tarball(
    exports: ExportsMap,
    files: Vec<(&str, &str)>,
  ) -> Result<HashMap<String, Vec<u8>>, anyhow::Error> {
    let package = PackageName::new("foo".to_string())?;
    let scope = ScopeName::new("deno-test".to_string())?;
    let version = Version::new("1.0.0")?;

    let mut memory_files = vec![];
    for file in files {
      let specifier = format!("file://{}", file.0);
      memory_files.push((
        specifier.clone(),
        Source::Module {
          specifier,
          maybe_headers: None,
          content: file.1.to_string(),
        },
      ));
    }

    let mut loader = MemoryLoader::new(memory_files, vec![]);
    let mut graph = ModuleGraph::new(GraphKind::All);
    let workspace_members = vec![WorkspaceMember {
      base: Url::parse("file:///").unwrap(),
      exports: exports.clone().into_inner(),
      nv: PackageNv {
        name: format!("@{}/{}", scope, package),
        version: version.0.clone(),
      },
    }];

    let mut roots: Vec<ModuleSpecifier> = vec![];
    for ex in exports.iter() {
      let raw = format!("file://{}", ex.1);
      let specifier = Url::parse(&raw).unwrap();
      roots.push(specifier);
    }

    let module_analyzer = ModuleAnalyzer::default();
    graph
      .build(
        roots,
        &mut loader,
        BuildOptions {
          is_dynamic: false,
          module_analyzer: Some(&module_analyzer),
          module_parser: Some(&module_analyzer.analyzer),
          workspace_members: &workspace_members,
          file_system: Some(&NullFileSystem),
          resolver: None,
          npm_resolver: None,
          reporter: None,
          ..Default::default()
        },
      )
      .await;

    let deps: Vec<(DependencyKind, PackageReqReference)> = vec![];

    let npm_tarball = create_npm_tarball(NpmTarballOptions {
      exports: &exports,
      package: &package,
      registry_url: &Url::parse("http://jsr.test").unwrap(),
      scope: &scope,
      version: &Version::new("1.0.0").unwrap(),
      graph: &graph,
      sources: &module_analyzer.analyzer,
      dependencies: deps.iter(),
    })?;

    let mut transpiled_files: HashMap<String, Vec<u8>> = HashMap::new();

    let mut gz_decoder =
      flate2::bufread::GzDecoder::new(&npm_tarball.tarball[..]);
    let mut raw = vec![];
    gz_decoder.read_to_end(&mut raw)?;
    let mut archive = Archive::new(&raw[..]).entries()?;

    while let Some(res) = archive.next().await {
      let mut entry = res.unwrap();

      let path = entry.path().unwrap().display().to_string();
      // For our tests we don't care about the package parent folder
      let len = "package".to_string().len();
      let formatted_path = path[len..].to_string();

      let mut buf = vec![];
      entry.read_to_end(&mut buf).await?;
      transpiled_files.insert(formatted_path, buf);
    }

    Ok(transpiled_files)
  }

  fn get_content_without_source_map(data: &[u8]) -> String {
    let s = String::from_utf8_lossy(data);
    if let Some(idx) = s.find("\n//# sourceMappingURL") {
      s[..idx].to_string()
    } else {
      s.to_string()
    }
  }

  #[tokio::test]
  async fn import_sources_test() -> Result<(), anyhow::Error> {
    let source = "import {html} from 'npm:lit@^2.2.7';console.log(html)";
    let files = vec![
      ("/package.json", ""),
      ("/foo.js", source),
      ("/bar.mjs", source),
    ];
    let exports = ExportsMap::new(IndexMap::from([
      (".".to_string(), "/foo.js".to_string()),
      ("./bar".to_string(), "/bar.mjs".to_string()),
    ]));
    let tarball_files = test_npm_tarball(exports, files).await?;

    let expected =
      "import { html } from \"lit\";\nconsole.log(html);".to_string();

    let foo_js =
      get_content_without_source_map(tarball_files.get("/foo.js").unwrap());
    let bar_mjs =
      get_content_without_source_map(tarball_files.get("/bar.mjs").unwrap());
    assert_eq!(foo_js, expected);
    assert_eq!(bar_mjs, expected);

    Ok(())
  }
}
