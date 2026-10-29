// Copyright 2024 the JSR authors. All rights reserved. MIT license.
use std::collections::HashMap;
use std::collections::HashSet;

use deno_graph::ModuleGraph;
use deno_graph::ast::CapturingModuleAnalyzer;
use deno_semver::package::PackageReqReference;
use jsr_npm_tarball::NpmPackageJsonOptions;
use jsr_npm_tarball::PackageFiles;
use url::Url;

use crate::db::DependencyKind;
use crate::db::ExportsMap;
use crate::ids::PackageName;
use crate::ids::PackagePath;
use crate::ids::ScopeName;
use crate::ids::Version;
use crate::s3::BucketWithQueue;

use super::NPM_TARBALL_REVISION;
use super::types::NpmMappedJsrPackageName;

pub use jsr_npm_tarball::NpmTarball;

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
    analyzer,
    registry_url,
    scope,
    package,
    version,
    exports,
    files,
    dependencies,
  } = opts;

  let npm_name = NpmMappedJsrPackageName { scope, package }.to_string();
  let homepage = Url::options()
    .base_url(Some(registry_url))
    .parse(&format!("./@{scope}/{package}",))
    .unwrap()
    .to_string();
  let dependencies = dependencies
    .map(|(kind, req)| (npm_dependency_kind(*kind), req.clone()))
    .collect::<Vec<_>>();
  let transpile_options =
    jsr_npm_tarball::transpile_options_from_compiler_options(None);

  let files = RegistryFiles {
    files,
    scope,
    package,
    version,
  };

  jsr_npm_tarball::create_npm_tarball(jsr_npm_tarball::NpmTarballOptions {
    graph,
    parser: &analyzer.as_capturing_parser(),
    package_json: NpmPackageJsonOptions {
      name: &npm_name,
      version: &version.0,
      homepage: Some(homepage),
      license: None,
      jsr_revision: Some(NPM_TARBALL_REVISION),
    },
    exports: exports
      .iter()
      .map(|(key, path)| (key.as_str(), path.as_str())),
    files: &files,
    dependencies: dependencies.iter(),
    transpile_options: &transpile_options,
    source_maps: true,
  })
  .await
}

pub fn npm_dependency_kind(
  kind: DependencyKind,
) -> jsr_npm_tarball::DependencyKind {
  match kind {
    DependencyKind::Jsr => jsr_npm_tarball::DependencyKind::Jsr,
    DependencyKind::Npm => jsr_npm_tarball::DependencyKind::Npm,
  }
}

struct RegistryFiles<'a> {
  files: NpmTarballFiles<'a>,
  scope: &'a ScopeName,
  package: &'a PackageName,
  version: &'a Version,
}

impl PackageFiles for RegistryFiles<'_> {
  fn paths(&self) -> impl Iterator<Item = &str> {
    let paths: Box<dyn Iterator<Item = &PackagePath>> = match &self.files {
      NpmTarballFiles::WithBytes(files) => Box::new(files.keys()),
      NpmTarballFiles::FromBucket { files, .. } => Box::new(files.iter()),
    };
    paths.map(|path| &**path)
  }

  async fn read(&self, path: &str) -> Result<Vec<u8>, anyhow::Error> {
    let path = PackagePath::new(path.to_string())?;
    match &self.files {
      NpmTarballFiles::WithBytes(files) => files
        .get(&path)
        .cloned()
        .ok_or_else(|| anyhow::anyhow!("file missing: {path}")),
      NpmTarballFiles::FromBucket { modules_bucket, .. } => {
        let s3_path = crate::s3_paths::file_path(
          self.scope,
          self.package,
          self.version,
          &path,
        )
        .into();
        let bytes = modules_bucket
          .download(s3_path)
          .await?
          .ok_or_else(|| anyhow::anyhow!("file missing on S3: {path}"))?;
        Ok(bytes.to_vec())
      }
    }
  }
}

#[cfg(test)]
mod tests {
  use std::collections::HashMap;
  use std::fmt::Write;
  use std::io::Read;

  use deno_ast::ModuleSpecifier;
  use deno_graph::BuildFastCheckTypeGraphOptions;
  use deno_graph::BuildOptions;
  use deno_graph::GraphKind;
  use deno_graph::ModuleGraph;
  use deno_graph::WorkspaceFastCheckOption;
  use deno_graph::WorkspaceMember;
  use deno_graph::source::MemoryLoader;
  use deno_graph::source::NullFileSystem;
  use deno_graph::source::Source;
  use deno_semver::package::PackageReqReference;
  use tar::Archive;
  use url::Url;

  use crate::analysis::JsrResolver;
  use crate::analysis::ModuleAnalyzer;
  use crate::analysis::PassthroughJsrUrlProvider;
  use crate::db::DependencyKind;
  use crate::ids::PackagePath;
  use crate::npm::NPM_TARBALL_REVISION;
  use crate::npm::tests::helpers;
  use crate::npm::tests::helpers::Spec;
  use crate::tarball::exports_map_from_json;

  use super::NpmTarballFiles;
  use super::NpmTarballOptions;
  use super::create_npm_tarball;

  async fn test_npm_tarball(
    spec_path: &Path,
    mut spec: Spec,
  ) -> Result<(), anyhow::Error> {
    let scope = spec.jsr_json.name.scope.clone();
    let package = spec.jsr_json.name.package.clone();
    let version = spec.jsr_json.version.clone().unwrap();

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
    let workspace_member = WorkspaceMember {
      base: Url::parse("file:///").unwrap(),
      name: StackString::from_string(format!("@{}/{}", scope, package)),
      version: Some(version.0.clone()),
      exports: exports.clone().into_inner(),
    };
    let workspace_members = vec![workspace_member.clone()];

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
        vec![],
        &loader,
        BuildOptions {
          is_dynamic: false,
          module_analyzer: &module_analyzer,
          file_system: &NullFileSystem,
          resolver: Some(&JsrResolver {
            member: workspace_member,
          }),
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
      es_parser: Some(&module_analyzer.analyzer),
      resolver: None,
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
    let mut ar = Archive::new(&raw[..]);
    let archive = ar.entries()?;

    for res in archive {
      let mut entry = res.unwrap();

      let path = entry.path().unwrap().display().to_string();
      // For our tests we don't care about the package parent folder
      let len = "package".to_string().len();
      let formatted_path = path[len..].to_string();

      let mode = entry.header().mode().unwrap();
      assert_eq!(
        mode, 0o644,
        "file {path} has unexpected mode {mode:o}, want 0o644",
      );

      let mut buf = vec![];
      entry.read_to_end(&mut buf)?;
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
      spec.output_file.text.clone_from(&output);
      std::fs::write(spec_path, spec.emit())?;
    } else {
      assert_eq!(
        output, spec.output_file.text,
        "Output not identical for {spec_path:?}, run with UPDATE=1 to update",
      );
    }

    Ok(())
  }

  use deno_semver::StackString;
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
