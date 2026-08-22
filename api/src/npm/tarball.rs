// Copyright 2024 the JSR authors. All rights reserved. MIT license.
use std::borrow::Cow;
use std::collections::HashMap;
use std::collections::HashSet;

use base64::Engine;
use deno_ast::SourceTextInfo;
use deno_ast::TextChange;
use deno_ast::apply_text_changes;
use deno_graph::ModuleGraph;
use deno_graph::ModuleSpecifier;
use deno_graph::PositionRange;
use deno_graph::Resolution;
use deno_graph::analysis::DependencyDescriptor;
use deno_graph::analysis::ModuleAnalyzer;
use deno_graph::analysis::ModuleInfo;
use deno_graph::ast::CapturingModuleAnalyzer;
use deno_graph::ast::ParsedSourceStore;
use deno_semver::package::PackageReqReference;
use futures::StreamExt;
use futures::TryStreamExt;
use indexmap::IndexMap;
use sha2::Digest;
use tar::Header;
use tracing::error;
use url::Url;

use crate::db::DependencyKind;
use crate::db::ExportsMap;
use crate::ids::PackageName;
use crate::ids::PackagePath;
use crate::ids::ScopeName;
use crate::ids::ScopedPackageName;
use crate::ids::Version;
use crate::s3::BucketWithQueue;

use super::NPM_TARBALL_REVISION;
use super::emit::transpile_to_dts;
use super::emit::transpile_to_js;
use super::specifiers::Extension;
use super::specifiers::RewriteKind;
use super::specifiers::SpecifierRewriter;
use super::specifiers::follow_specifier;
use super::specifiers::relative_import_specifier;
use super::specifiers::rewrite_file_specifier;
use super::types::NpmExportConditions;
use super::types::NpmMappedJsrPackageName;
use super::types::NpmPackageJson;

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
  pub analyzer: &'a CapturingModuleAnalyzer,
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
    analyzer: sources,
    registry_url,
    scope,
    package,
    version,
    exports,
    files,
    dependencies,
  } = opts;

  let npm_package_id = NpmMappedJsrPackageName { scope, package };

  let npm_dependencies =
    create_npm_dependencies(dependencies.map(Cow::Borrowed))?;

  let homepage = Url::options()
    .base_url(Some(registry_url))
    .parse(&format!("./@{scope}/{package}",))
    .unwrap()
    .to_string();

  let mut package_files = IndexMap::new();
  let mut to_be_rewritten = vec![];

  let mut source_rewrites = HashMap::<&ModuleSpecifier, ModuleSpecifier>::new();
  let mut declaration_rewrites =
    HashMap::<&ModuleSpecifier, ModuleSpecifier>::new();

  for module in graph.modules() {
    if module.specifier().scheme() != "file" {
      continue;
    };

    let Some(js) = module.js() else { continue };

    match js.media_type {
      deno_ast::MediaType::JavaScript | deno_ast::MediaType::Mjs => {
        if let Some(types_dep) = &js.maybe_types_dependency
          && let Resolution::Ok(resolved) = &types_dep.dependency
        {
          declaration_rewrites
            .insert(module.specifier(), resolved.specifier.clone());
        }
      }
      deno_ast::MediaType::Jsx => {
        let source_specifier =
          rewrite_file_specifier(module.specifier(), "", Extension::Js);
        if let Some(source_specifier) = source_specifier.clone() {
          source_rewrites.insert(module.specifier(), source_specifier);
        }

        if let Some(types_dep) = &js.maybe_types_dependency
          && let Resolution::Ok(resolved) = &types_dep.dependency
        {
          declaration_rewrites
            .insert(module.specifier(), resolved.specifier.clone());
        } else if let Some(source_specifier) = source_specifier {
          declaration_rewrites.insert(module.specifier(), source_specifier);
        }
      }
      deno_ast::MediaType::Dts | deno_ast::MediaType::Dmts => {}
      deno_ast::MediaType::TypeScript
      | deno_ast::MediaType::Mts
      | deno_ast::MediaType::Tsx => {
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
      deno_ast::MediaType::JavaScript | deno_ast::MediaType::Mjs => {
        let parsed_source = sources.get_parsed_source(&js.specifier).unwrap();
        let module_info = sources
          .analyze(&js.specifier, js.source.text.clone(), js.media_type)
          .await
          .unwrap();
        let specifier_rewriter = SpecifierRewriter {
          base_specifier: &js.specifier,
          source_rewrites: &source_rewrites,
          declaration_rewrites: &declaration_rewrites,
          dependencies: &js.dependencies,
        };
        let rewritten = rewrite_specifiers(
          parsed_source.text_info_lazy(),
          &module_info,
          specifier_rewriter,
          RewriteKind::Source,
        );
        package_files
          .insert(js.specifier.path().to_owned(), rewritten.into_bytes());
      }
      deno_ast::MediaType::Dts | deno_ast::MediaType::Dmts => {
        let parsed_source = sources.get_parsed_source(&js.specifier).unwrap();
        let module_info = sources
          .analyze(&js.specifier, js.source.text.clone(), js.media_type)
          .await
          .unwrap();
        let specifier_rewriter = SpecifierRewriter {
          base_specifier: &js.specifier,
          source_rewrites: &source_rewrites,
          declaration_rewrites: &declaration_rewrites,
          dependencies: &js.dependencies,
        };
        let rewritten = rewrite_specifiers(
          parsed_source.text_info_lazy(),
          &module_info,
          specifier_rewriter,
          RewriteKind::Declaration,
        );
        package_files
          .insert(js.specifier.path().to_owned(), rewritten.into_bytes());
      }
      deno_ast::MediaType::Jsx => {
        let parsed_source = sources.get_parsed_source(&js.specifier).unwrap();
        let source_target = source_rewrites.get(&js.specifier).unwrap();
        let specifier_rewriter = SpecifierRewriter {
          base_specifier: source_target,
          source_rewrites: &source_rewrites,
          declaration_rewrites: &declaration_rewrites,
          dependencies: &js.dependencies,
        };
        let (source, source_map) =
          transpile_to_js(&parsed_source, specifier_rewriter, source_target)
            .unwrap();
        package_files.insert(source_target.path().to_owned(), source);
        package_files
          .insert(format!("{}.map", source_target.path()), source_map);
      }
      deno_ast::MediaType::TypeScript
      | deno_ast::MediaType::Mts
      | deno_ast::MediaType::Tsx => {
        let parsed_source = sources.get_parsed_source(&js.specifier).unwrap();
        let module_info = sources
          .analyze(&js.specifier, js.source.text.clone(), js.media_type)
          .await
          .unwrap();
        let specifier_rewriter = SpecifierRewriter {
          base_specifier: &js.specifier,
          source_rewrites: &source_rewrites,
          declaration_rewrites: &declaration_rewrites,
          dependencies: &js.dependencies,
        };
        let rewritten = rewrite_specifiers(
          parsed_source.text_info_lazy(),
          &module_info,
          specifier_rewriter,
          RewriteKind::Source,
        );
        package_files
          .insert(js.specifier.path().to_owned(), rewritten.into_bytes());

        let parsed_source = sources.get_parsed_source(&js.specifier).unwrap();
        let source_target = source_rewrites.get(&js.specifier).unwrap();
        let specifier_rewriter = SpecifierRewriter {
          base_specifier: source_target,
          source_rewrites: &source_rewrites,
          declaration_rewrites: &declaration_rewrites,
          dependencies: &js.dependencies,
        };
        let (source, source_map) =
          transpile_to_js(&parsed_source, specifier_rewriter, source_target)
            .unwrap();
        package_files.insert(source_target.path().to_owned(), source);
        package_files
          .insert(format!("{}.map", source_target.path()), source_map);

        if let Some(fast_check_module) = js.fast_check_module() {
          let declaration_target =
            declaration_rewrites.get(&js.specifier).unwrap();
          let specifier_rewriter = SpecifierRewriter {
            base_specifier: declaration_target,
            source_rewrites: &source_rewrites,
            declaration_rewrites: &declaration_rewrites,
            dependencies: &js.dependencies,
          };
          let (declaration, declaration_map) = transpile_to_dts(
            &parsed_source,
            fast_check_module,
            specifier_rewriter,
            declaration_target,
          )?;
          package_files
            .insert(declaration_target.path().to_owned(), declaration);
          package_files.insert(
            format!("{}.map", declaration_target.path()),
            declaration_map,
          );
        }
      }
      _ => {}
    }
  }

  match files {
    NpmTarballFiles::WithBytes(files) => {
      for (path, content) in files.iter() {
        if !package_files.contains_key(&**path) {
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
        if !package_files.contains_key(&**path) {
          paths_to_download.push(path);
        }
      }

      let downloads = futures::stream::iter(paths_to_download.into_iter())
        .map(|path| {
          let s3_path =
            crate::s3_paths::file_path(scope, package, version, path).into();
          async move {
            let bytes = modules_bucket
              .download(s3_path)
              .await?
              .ok_or_else(|| anyhow::anyhow!("file missing on S3: {path}"))?;
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

  let npm_exports = create_npm_exports(
    exports,
    &package_files,
    &source_rewrites,
    &declaration_rewrites,
  );

  let pkg_json = NpmPackageJson {
    name: npm_package_id,
    version: version.clone(),
    module_type: "module".to_string(),
    exports: npm_exports,
    dependencies: npm_dependencies,
    homepage,
    revision: NPM_TARBALL_REVISION,
  };

  let pkg_json_str = serde_json::to_string_pretty(&pkg_json)?;
  package_files.insert("/package.json".to_string(), pkg_json_str.into());

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
      error!("bad npm tarball path {} {}", path, e);
      crate::tarball::PublishError::InvalidPath {
        path: path.to_string(),
        error: crate::ids::PackagePathValidationError::TooLong(path.len()),
      }
    })?;
    header.set_size(content.len() as u64);
    header.set_mode(0o644);
    header.set_mtime(mtime);
    header.set_cksum();
    tarball.append(&header, content.as_slice()).unwrap();
  }

  tarball.into_inner().unwrap();
  gz_encoder.finish().unwrap();

  drop(package_files);

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
