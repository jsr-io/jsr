// Copyright 2024 the JSR authors. All rights reserved. MIT license.
use std::borrow::Cow;
use std::collections::HashMap;
use std::collections::HashSet;

use base64::Engine;
use deno_ast::apply_text_changes;
use deno_ast::SourceTextInfo;
use deno_ast::TextChange;
use deno_graph::CapturingModuleAnalyzer;
use deno_graph::DependencyDescriptor;
use deno_graph::ModuleAnalyzer;
use deno_graph::ModuleGraph;
use deno_graph::ModuleInfo;
use deno_graph::ModuleSpecifier;
use deno_graph::ParsedSourceStore;
use deno_graph::PositionRange;
use deno_graph::Resolution;
use deno_semver::package::PackageReqReference;
use futures::StreamExt;
use futures::TryStreamExt;
use indexmap::IndexMap;
use sha2::Digest;
use tar::Header;
use tracing::error;
use url::Url;

use crate::buckets::BucketWithQueue;
use crate::db::DependencyKind;
use crate::db::ExportsMap;
use crate::ids::PackageName;
use crate::ids::PackagePath;
use crate::ids::ScopeName;
use crate::ids::ScopedPackageName;
use crate::ids::Version;

use super::emit::transpile_to_dts;
use super::emit::transpile_to_js;
use super::specifiers::follow_specifier;
use super::specifiers::relative_import_specifier;
use super::specifiers::rewrite_file_specifier_extension;
use super::specifiers::Extension;
use super::specifiers::RewriteKind;
use super::specifiers::SpecifierRewriter;
use super::types::NpmExportConditions;
use super::types::NpmMappedJsrPackageName;
use super::types::NpmPackageJson;
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

  // Mapping of original specifiers in the module graph to where one can find
  // the source code or declarations for that module in the tarball, if it
  // differs from the original specifier.
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
        if let Some(types_dep) = &js.maybe_types_dependency {
          if let Resolution::Ok(resolved) = &types_dep.dependency {
            declaration_rewrites
              .insert(module.specifier(), resolved.specifier.clone());
          }
        }
      }
      deno_ast::MediaType::Jsx => {
        let source_specifier =
          rewrite_file_specifier_extension(module.specifier(), Extension::Js);
        if let Some(source_specifier) = source_specifier {
          source_rewrites.insert(module.specifier(), source_specifier);
        }

        if let Some(types_dep) = &js.maybe_types_dependency {
          if let Resolution::Ok(resolved) = &types_dep.dependency {
            declaration_rewrites
              .insert(module.specifier(), resolved.specifier.clone());
          }
        }
      }
      deno_ast::MediaType::Dts | deno_ast::MediaType::Dmts => {
        // no extra work needed for these, as they can not have type dependencies
      }
      deno_ast::MediaType::TypeScript | deno_ast::MediaType::Mts => {
        let source_specifier =
          rewrite_file_specifier_extension(module.specifier(), Extension::Js);
        if let Some(source_specifier) = source_specifier.clone() {
          source_rewrites.insert(module.specifier(), source_specifier);
        }

        if js.fast_check_module().is_some() {
          let declaration_specifier = rewrite_file_specifier_extension(
            module.specifier(),
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
    let specifier_rewriter = SpecifierRewriter {
      base_specifier: &js.specifier,
      source_rewrites: &source_rewrites,
      declaration_rewrites: &declaration_rewrites,
      dependencies: &js.dependencies,
    };

    match js.media_type {
      deno_ast::MediaType::JavaScript | deno_ast::MediaType::Mjs => {
        let parsed_source = sources.get_parsed_source(&js.specifier).unwrap();
        let module_info = sources
          .analyze(&js.specifier, js.source.clone(), js.media_type)
          .unwrap();
        let rewritten = rewrite_specifiers(
          parsed_source.text_info(),
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
          .analyze(&js.specifier, js.source.clone(), js.media_type)
          .unwrap();
        let rewritten = rewrite_specifiers(
          parsed_source.text_info(),
          &module_info,
          specifier_rewriter,
          RewriteKind::Declaration,
        );
        package_files
          .insert(js.specifier.path().to_owned(), rewritten.into_bytes());
      }
      deno_ast::MediaType::Jsx => {
        let parsed_source = sources.get_parsed_source(&js.specifier).unwrap();
        let source =
          transpile_to_js(&parsed_source, specifier_rewriter).unwrap();
        let source_target = source_rewrites.get(&js.specifier).unwrap();
        package_files
          .insert(source_target.path().to_owned(), source.into_bytes());
      }
      deno_ast::MediaType::TypeScript | deno_ast::MediaType::Mts => {
        let parsed_source = sources.get_parsed_source(&js.specifier).unwrap();
        let module_info = sources
          .analyze(&js.specifier, js.source.clone(), js.media_type)
          .unwrap();
        let rewritten = rewrite_specifiers(
          parsed_source.text_info(),
          &module_info,
          specifier_rewriter,
          RewriteKind::Source,
        );
        package_files
          .insert(js.specifier.path().to_owned(), rewritten.into_bytes());

        let parsed_source = sources.get_parsed_source(&js.specifier).unwrap();
        let source =
          transpile_to_js(&parsed_source, specifier_rewriter).unwrap();
        let source_target = source_rewrites.get(&js.specifier).unwrap();
        package_files
          .insert(source_target.path().to_owned(), source.into_bytes());

        if let Some(fast_check_module) = js.fast_check_module() {
          let declaration = transpile_to_dts(
            &parsed_source,
            fast_check_module,
            specifier_rewriter,
          )?;
          let declaration_target =
            declaration_rewrites.get(&js.specifier).unwrap();
          package_files.insert(
            declaration_target.path().to_owned(),
            declaration.into_bytes(),
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

fn rewrite_specifiers(
  source_text_info: &SourceTextInfo,
  module_info: &ModuleInfo,
  specifier_rewriter: SpecifierRewriter,
  kind: RewriteKind,
) -> String {
  let mut text_changes = vec![];

  let add_text_change = |text_changes: &mut Vec<TextChange>,
                         new_specifier: String,
                         range: &PositionRange| {
    let start_pos = source_text_info.range().start;
    let mut start = range
      .start
      .as_source_pos(source_text_info)
      .as_byte_index(start_pos);
    let mut end = range
      .end
      .as_source_pos(source_text_info)
      .as_byte_index(start_pos);

    let to_be_replaced = &source_text_info.text_str()[start..end];
    if to_be_replaced.starts_with('\'')
      || to_be_replaced.starts_with('"')
      || to_be_replaced.starts_with('`')
    {
      start += 1;
      end -= 1;
    }

    text_changes.push(TextChange {
      new_text: new_specifier,
      range: start..end,
    });
  };

  for desc in &module_info.dependencies {
    match desc {
      DependencyDescriptor::Static(desc) => {
        if let Some(specifier) =
          specifier_rewriter.rewrite(&desc.specifier, kind)
        {
          add_text_change(&mut text_changes, specifier, &desc.specifier_range);
        }
      }
      DependencyDescriptor::Dynamic(desc) => match &desc.argument {
        deno_graph::DynamicArgument::String(specifier) => {
          if let Some(specifier) = specifier_rewriter.rewrite(specifier, kind) {
            add_text_change(&mut text_changes, specifier, &desc.argument_range);
          }
        }
        deno_graph::DynamicArgument::Template(_) => {}
        deno_graph::DynamicArgument::Expr => {}
      },
    }
  }

  for ts_ref in &module_info.ts_references {
    match ts_ref {
      deno_graph::TypeScriptReference::Path(s) => {
        if let Some(specifier) =
          specifier_rewriter.rewrite(&s.text, RewriteKind::Declaration)
        {
          add_text_change(&mut text_changes, specifier, &s.range);
        }
      }
      deno_graph::TypeScriptReference::Types(s) => {
        match kind {
          RewriteKind::Source => {
            // Type reference comments in JS are a Deno specific concept, and
            // are thus not relevant for the tarball. We remove them.

            let start_pos = source_text_info.range().start;
            let start = s.range.start.as_source_pos(source_text_info);
            let start = source_text_info.line_and_column_index(start);

            let line_start = source_text_info.line_start(start.line_index);
            let line_end = source_text_info.line_end(start.line_index);
            let line_text = source_text_info.line_text(start.line_index);

            let before = line_text[..start.column_index].to_string();

            let index = before.rfind("///").expect("should have ///");
            let comment_start = line_start + index;
            let comment_end = line_end;

            let range = comment_start.as_byte_index(start_pos)
              ..comment_end.as_byte_index(start_pos);

            text_changes.push(TextChange {
              new_text: "".to_string(),
              range,
            });
          }
          RewriteKind::Declaration => {
            if let Some(specifier) =
              specifier_rewriter.rewrite(&s.text, RewriteKind::Declaration)
            {
              add_text_change(&mut text_changes, specifier, &s.range);
            }
          }
        }
      }
    }
  }

  for s in &module_info.jsdoc_imports {
    if let Some(specifier) =
      specifier_rewriter.rewrite(&s.text, RewriteKind::Declaration)
    {
      add_text_change(&mut text_changes, specifier, &s.range);
    }
  }

  apply_text_changes(source_text_info.text_str(), text_changes)
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

pub fn create_npm_exports(
  exports: &ExportsMap,
  package_files: &IndexMap<String, Vec<u8>>,
  source_rewrites: &HashMap<&ModuleSpecifier, ModuleSpecifier>,
  declaration_rewrites: &HashMap<&ModuleSpecifier, ModuleSpecifier>,
) -> IndexMap<String, NpmExportConditions> {
  let package_json_specifier =
    ModuleSpecifier::parse("file:///package.json").unwrap();

  let mut npm_exports = IndexMap::new();
  for (key, path) in exports.iter() {
    let mut conditions = NpmExportConditions {
      types: None,
      default: None,
    };

    let specifier = ModuleSpecifier::parse(&format!(
      "file:///{}",
      path.trim_start_matches('.').trim_start_matches('/')
    ))
    .unwrap();

    if let Some(source_specifier) =
      follow_specifier(&specifier, source_rewrites)
    {
      if source_specifier.scheme() == "file"
        && package_files.contains_key(source_specifier.path())
      {
        let new_specifier =
          relative_import_specifier(&package_json_specifier, source_specifier);
        conditions.default = Some(new_specifier);
      }
    }

    if let Some(types_specifier) =
      follow_specifier(&specifier, declaration_rewrites)
    {
      if types_specifier.scheme() == "file"
        && package_files.contains_key(types_specifier.path())
      {
        let new_specifier =
          relative_import_specifier(&package_json_specifier, types_specifier);
        if conditions.default.as_ref() != Some(&new_specifier) {
          conditions.types = Some(new_specifier);
        }
      }
    }

    npm_exports.insert(key.clone(), conditions);
  }
  npm_exports
}

#[cfg(test)]
mod tests {
  use std::collections::HashMap;
  use std::fmt::Write;
  use std::io::Read;

  use async_tar::Archive;
  use deno_ast::ModuleSpecifier;
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
  use url::Url;

  use crate::analysis::ModuleAnalyzer;
  use crate::analysis::PassthroughJsrUrlProvider;
  use crate::db::DependencyKind;
  use crate::ids::PackagePath;
  use crate::npm::tests::helpers;
  use crate::npm::tests::helpers::Spec;
  use crate::npm::NPM_TARBALL_REVISION;
  use crate::tarball::exports_map_from_json;

  use super::create_npm_tarball;
  use super::NpmTarballFiles;
  use super::NpmTarballOptions;

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

    let loader = MemoryLoader::new(memory_files, vec![]);
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
      let raw = format!("file://{}", ex.1.strip_prefix('.').unwrap());
      let specifier = Url::parse(&raw).unwrap();
      roots.push(specifier);
    }

    let module_analyzer = ModuleAnalyzer::default();
    graph
      .build(
        roots,
        &loader,
        BuildOptions {
          is_dynamic: false,
          module_analyzer: &module_analyzer,
          workspace_members: &workspace_members,
          file_system: &NullFileSystem,
          resolver: None,
          npm_resolver: None,
          reporter: None,
          jsr_url_provider: &PassthroughJsrUrlProvider,
          passthrough_jsr_specifiers: true,
          ..Default::default()
        },
      )
      .await;
    graph.valid()?;
    graph.build_fast_check_type_graph(BuildFastCheckTypeGraphOptions {
      fast_check_cache: Default::default(),
      fast_check_dts: true,
      jsr_url_provider: &PassthroughJsrUrlProvider,
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
      analyzer: &module_analyzer.analyzer,
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
      let content = content.replace(
        &format!("\"_jsr_revision\": {NPM_TARBALL_REVISION}"),
        "\"_jsr_revision\": 0",
      );
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
        .unwrap_or_else(|e| panic!("failed to test npm tarball {path:?}: {e}"));
    }
  }
}
