// Copyright 2024 the JSR authors. All rights reserved. MIT license.

//! Creates the npm tarball of a JSR package: TypeScript is transpiled to
//! JavaScript, declarations are generated from the fast check graph, and
//! `jsr:` and `npm:` specifiers are rewritten to npm package names.

mod emit;
mod graph;
mod import_transform;
mod specifiers;
mod tarball;

pub use self::emit::transpile_options_from_compiler_options;
pub use self::graph::JsrResolver;
pub use self::graph::PackageGraph;
pub use self::graph::PassthroughJsrUrlProvider;
pub use self::graph::build_package_graph;
pub use self::specifiers::jsr_to_npm_package_name;
pub use self::tarball::DependencyKind;
pub use self::tarball::NpmPackageJsonOptions;
pub use self::tarball::NpmTarball;
pub use self::tarball::NpmTarballEntry;
pub use self::tarball::NpmTarballOptions;
pub use self::tarball::PackageFiles;
pub use self::tarball::create_npm_dependencies;
pub use self::tarball::create_npm_tarball;

/// The npm scope that JSR packages are published under on the JSR npm
/// registry: `@jsr/<scope>__<name>`.
pub const NPM_SCOPE: &str = "jsr";
