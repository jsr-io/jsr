// Copyright 2024 the JSR authors. All rights reserved. MIT license.
use std::borrow::Cow;
use std::collections::HashMap;
use std::collections::HashSet;

use anyhow::Context;
use base64::Engine;
use deno_ast::apply_text_changes;
use deno_ast::ParsedSource;
use deno_ast::TextChange;
use deno_graph::DependencyDescriptor;
use deno_graph::ModuleGraph;
use deno_graph::ParsedSourceStore;
use deno_graph::ParserModuleAnalyzer;
use deno_graph::PositionRange;
use deno_semver::package::PackageReqReference;
use futures::StreamExt;
use futures::TryStreamExt;
use indexmap::IndexMap;
use sha2::Digest;
use tar::Header;
use tracing::error;
use tracing::info;
use url::Url;

use crate::buckets::BucketWithQueue;
use crate::db::DependencyKind;
use crate::db::ExportsMap;
use crate::ids::PackageName;
use crate::ids::PackagePath;
use crate::ids::ScopeName;
use crate::ids::ScopedPackageName;
use crate::ids::Version;
use crate::npm::specifiers::rewrite_extension;
use crate::npm::specifiers::rewrite_specifier;
use crate::npm::specifiers::Extension;
use crate::npm::types::NpmMappedJsrPackageName;
use crate::npm::types::NpmPackageJson;

use super::emit::transpile_to_js;
use super::NPM_TARBALL_REVISION;

pub struct NpmTarball {
  /// The gzipped tarball contents.
  pub tarball: Vec<u8>,
  /// The hex encoded sha1 hash of the gzipped tarball.
  pub sha1: String,
  /// The base64 encoded sha512 hash of the gzipped tarball.
  pub sha512: String,
}

pub enum NpmTarballFiles<'a> {
  WithBytes(&'a HashMap<PackagePath, Vec<u8>>),
  FromBucket {
    files: &'a HashSet<PackagePath>,
    modules_bucket: &'a BucketWithQueue,
  },
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
  pub files: NpmTarballFiles<'a>,
  pub dependencies: Deps,
}

pub async fn create_npm_tarball<'a>(
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
    files,
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
    revision: NPM_TARBALL_REVISION,
  };

  let mut transpiled_files = HashSet::new();
  let mut package_files = IndexMap::new();

  for module in graph.modules() {
    if module.specifier().scheme() != "file" {
      continue;
    };
    let path = module.specifier().path();

    if let Some(json) = module.json() {
      package_files.insert(path.to_owned(), json.source.as_bytes().to_vec());
    } else if let Some(js) = module.js() {
      match js.media_type {
        // We need to rewrite import source in js files too
        // from `npm:*` to bare specifiers, for example.
        deno_ast::MediaType::JavaScript | deno_ast::MediaType::Mjs => {
          let source = sources
            .get_parsed_source(module.specifier())
            .expect("parsed source should be here");

          let module_info = ParserModuleAnalyzer::module_info(&source);

          let maybe_rewrite_specifier =
            |specifier: &str,
             range: &PositionRange,
             text_changes: &mut Vec<TextChange>| {
              if let Some(rewritten) = rewrite_specifier(specifier) {
                text_changes.push(TextChange {
                  new_text: rewritten,
                  range: to_range(&source, range),
                });
              }
            };

          let mut text_changes = vec![];
          for dep in &module_info.dependencies {
            match dep {
              DependencyDescriptor::Static(dep) => {
                maybe_rewrite_specifier(
                  &dep.specifier,
                  &dep.specifier_range,
                  &mut text_changes,
                );
              }
              DependencyDescriptor::Dynamic(dep) => match &dep.argument {
                deno_graph::DynamicArgument::String(str_arg) => {
                  maybe_rewrite_specifier(
                    str_arg,
                    &dep.argument_range,
                    &mut text_changes,
                  );
                }
                deno_graph::DynamicArgument::Template(_) => {}
                deno_graph::DynamicArgument::Expr => {}
              },
            }
          }

          let rewritten =
            apply_text_changes(source.text_info().text_str(), text_changes);

          package_files.insert(path.to_owned(), rewritten.as_bytes().to_vec());
        }
        deno_ast::MediaType::Dts | deno_ast::MediaType::Dmts => {
          package_files.insert(path.to_owned(), js.source.as_bytes().to_vec());
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
          transpiled_files.insert(path.to_owned());
          package_files.insert(rewritten_path, transpiled.as_bytes().to_vec());
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
          package_files.insert(rewritten_path, dts.text.as_bytes().to_vec());
        }
      }
    }
  }

  let pkg_json_str = serde_json::to_string_pretty(&pkg_json)?;
  package_files.insert("/package.json".to_string(), pkg_json_str.into());

  match files {
    NpmTarballFiles::WithBytes(files) => {
      for (path, content) in files.iter() {
        if !package_files.contains_key(&**path)
          && !transpiled_files.contains(&**path)
        {
          package_files.insert(path.to_string(), content.clone());
        }
      }
    }
    NpmTarballFiles::FromBucket {
      files,
      modules_bucket,
    } => {
      let mut paths_to_download = vec![];
      for path in files.iter() {
        if !package_files.contains_key(&**path)
          && !transpiled_files.contains(&**path)
        {
          paths_to_download.push(path);
        }
      }

      let downloads = futures::stream::iter(paths_to_download.into_iter())
        .map(|path| {
          let gcs_path =
            crate::gcs_paths::file_path(scope, package, version, path).into();
          async move {
            let bytes = modules_bucket
              .download(gcs_path)
              .await?
              .ok_or_else(|| anyhow::anyhow!("file missing on GCS: {path}"))?;
            Ok::<_, anyhow::Error>((path, bytes))
          }
        })
        .buffer_unordered(64);

      let downloaded_files = downloads.try_collect::<Vec<_>>().await?;
      for (path, content) in downloaded_files {
        package_files.insert(path.to_string(), content.to_vec());
      }
    }
  }

  package_files.sort_keys();

  let mut tar_gz_bytes = Vec::new();
  let mut gz_encoder = flate2::write::GzEncoder::new(
    &mut tar_gz_bytes,
    flate2::Compression::default(),
  );
  let mut tarball = tar::Builder::new(&mut gz_encoder);

  let now = std::time::SystemTime::now();
  let mtime = now.duration_since(std::time::UNIX_EPOCH).unwrap().as_secs();

  for (path, content) in package_files.iter() {
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

fn to_range(
  parsed_source: &ParsedSource,
  range: &PositionRange,
) -> std::ops::Range<usize> {
  let mut range = range
    .as_source_range(parsed_source.text_info())
    .as_byte_range(parsed_source.text_info().range().start);
  let text = &parsed_source.text_info().text_str()[range.clone()];
  if text.starts_with('"') || text.starts_with('\'') {
    range.start += 1;
  }
  if text.ends_with('"') || text.ends_with('\'') {
    range.end -= 1;
  }
  range
}

#[cfg(test)]
mod tests {
  use std::collections::HashMap;
  use std::fmt::Write;
  use std::io::Read;

  use async_tar::Archive;
  use deno_ast::ModuleSpecifier;
  use deno_graph::source::JsrUrlProvider;
  use deno_graph::source::MemoryLoader;
  use deno_graph::source::NullFileSystem;
  use deno_graph::source::Source;
  use deno_graph::BuildFastCheckTypeGraphOptions;
  use deno_graph::BuildOptions;
  use deno_graph::GraphKind;
  use deno_graph::ModuleGraph;
  use deno_graph::WorkspaceFastCheckOption;
  use deno_graph::WorkspaceMember;
  use deno_semver::package::PackageNv;
  use deno_semver::package::PackageReqReference;
  use futures::AsyncReadExt;
  use futures::StreamExt;
  use once_cell::sync::Lazy;
  use url::Url;

  use crate::analysis::ModuleAnalyzer;
  use crate::db::DependencyKind;
  use crate::ids::PackagePath;
  use crate::npm::tests::helpers;
  use crate::npm::tests::helpers::Spec;
  use crate::tarball::exports_map_from_json;

  use super::create_npm_tarball;
  use super::NpmTarballFiles;
  use super::NpmTarballOptions;

  pub static DEFAULT_JSR_TEST_URL: Lazy<Url> =
    Lazy::new(|| Url::parse("http://jsr.test").unwrap());

  struct JsrTestUrlProvider;

  impl JsrUrlProvider for JsrTestUrlProvider {
    fn url(&self) -> &Url {
      &DEFAULT_JSR_TEST_URL
    }
  }

  async fn test_npm_tarball(
    spec_path: &Path,
    mut spec: Spec,
  ) -> Result<(), anyhow::Error> {
    let scope = spec.jsr_json.name.scope.clone();
    let package = spec.jsr_json.name.package.clone();
    let version = spec.jsr_json.version.clone();

    let exports = match exports_map_from_json(spec.jsr_json.exports.clone()) {
      Ok(exports) => exports,
      Err(e) => {
        return Err(anyhow::anyhow!("failed to parse exports: {}", e));
      }
    };

    let mut files = HashMap::new();
    let mut memory_files = vec![];
    for file in &spec.files {
      let specifier = file.url();
      if file.text.trim() == "<external>" {
        memory_files.push((
          specifier.to_string(),
          Source::External(specifier.to_string()),
        ));
      } else {
        memory_files.push((
          specifier.to_string(),
          Source::Module {
            specifier: specifier.to_string(),
            maybe_headers: None,
            content: file.text.to_string(),
          },
        ));
      }
      if specifier.scheme() == "file" {
        files.insert(
          PackagePath::new(specifier.path().to_string()).unwrap(),
          file.text.as_bytes().to_vec(),
        );
      }
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
      let raw = format!("file://{}", ex.1.strip_prefix(".").unwrap());
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
          module_analyzer: &module_analyzer,
          workspace_members: &workspace_members,
          file_system: &NullFileSystem,
          resolver: None,
          npm_resolver: None,
          reporter: None,
          jsr_url_provider: &JsrTestUrlProvider,
          ..Default::default()
        },
      )
      .await;
    graph.valid()?;
    graph.build_fast_check_type_graph(BuildFastCheckTypeGraphOptions {
      fast_check_cache: Default::default(),
      fast_check_dts: true,
      jsr_url_provider: Some(&JsrTestUrlProvider),
      module_parser: Some(&module_analyzer.analyzer),
      resolver: None,
      npm_resolver: None,
      workspace_fast_check: WorkspaceFastCheckOption::Enabled(
        &workspace_members,
      ),
    });

    let deps: Vec<(DependencyKind, PackageReqReference)> = vec![];

    let npm_tarball = create_npm_tarball(NpmTarballOptions {
      exports: &exports,
      package: &package,
      registry_url: &Url::parse("http://jsr.test").unwrap(),
      scope: &scope,
      version: &version,
      graph: &graph,
      sources: &module_analyzer.analyzer,
      files: NpmTarballFiles::WithBytes(&files),
      dependencies: deps.iter(),
    })
    .await?;

    let mut transpiled_files: Vec<(String, Vec<u8>)> = Vec::new();

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
      transpiled_files.push((formatted_path, buf));
    }

    transpiled_files.sort_by(|a, b| a.0.cmp(&b.0));

    let mut output = String::new();
    for (path, content) in transpiled_files {
      let content = String::from_utf8_lossy(&content);
      write!(
        &mut output,
        "== {path} ==\n{}\n{}",
        content,
        if content.ends_with('\n') { "" } else { "\n" }
      )?;
    }

    if std::env::var("UPDATE").is_ok() {
      spec.output_file.text = output.clone();
      std::fs::write(spec_path, spec.emit())?;
    } else {
      assert_eq!(
        output, spec.output_file.text,
        "Output not identical for {spec_path:?}, run with UPDATE=1 to update",
      );
    }

    Ok(())
  }

  use std::path::Path;

  #[tokio::test]
  async fn test_npm_tarballs() {
    let specs =
      helpers::get_specs_in_dir(Path::new("testdata/specs/npm_tarballs"));
    for (path, spec) in specs {
      test_npm_tarball(&path, spec)
        .await
        .expect(&format!("failed to test npm tarball {}", path.display()));
    }
  }
}
