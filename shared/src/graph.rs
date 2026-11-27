// Copyright 2024 the JSR authors. All rights reserved. MIT license.
//! Pieces for building a `deno_graph` module graph of a package's own files:
//! used by the publish analysis in the tasks crate and by the api's
//! dependency tree endpoint.

use deno_ast::ModuleSpecifier;
use deno_ast::ParsedSource;
use deno_error::JsErrorBox;
use deno_graph::WorkspaceMember;
use deno_graph::ast::DefaultEsParser;
use deno_graph::source::JsrUrlProvider;
use deno_semver::jsr::JsrPackageReqReference;
use deno_semver::package::PackageNv;
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

#[derive(Default)]
pub struct ModuleParser(DefaultEsParser);

impl deno_graph::ast::EsParser for ModuleParser {
  fn parse_program(
    &self,
    options: deno_graph::ast::ParseOptions,
  ) -> Result<ParsedSource, deno_ast::ParseDiagnostic> {
    let source = self.0.parse_program(options)?;
    if let Some(err) = source.diagnostics().first() {
      return Err(err.clone());
    }
    Ok(source)
  }
}
