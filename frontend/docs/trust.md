---
description: JSR automatically creates provenance attestations using SLSA and Sigstore when a package is published from GitHub Actions.
---

Provenance statements can be a useful tool for understanding the security and
trustworthiness of a package. They provide a way to verify that a package was
built from the expected source code, and that it was published by the expected
person or organization.

JSR automatically creates provenance statements for each package that is
published from GitHub Actions. These statements are created using the
[Supply Chain Levels for Software Artifacts (SLSA)](https://slsa.dev) framework,
and are stored in the [Sigstore](https://sigstore.dev) Rekor transparency log.

To publish provenance for a package, you must publish the package from a GitHub
Actions workflow. The workflow must use the `jsr publish` or `deno publish`
command to publish the package. You must use the native JSR + GitHub Actions
publishing integration as described in the
[publishing guide](/docs/publishing-packages#publishing-from-github-actions). If
these conditions are met, JSR will automatically create a provenance statement
for the package.

You can opt out of creating provenance statements for a package by setting the
`--no-provenance` flag when publishing the package.

You can view the provenance statement for a package by visiting the package page
on jsr.io. At the bottom of the overview tab, you will see a "Provenance"
section. This section will contain a link to the Sigstore transparency log entry
for the package.

## Future support

In the future, JSR will additionally sign the uploaded package manifest and
publish this signature to the Sigstore transparency log. This will provide a way
to verify that the package manifest was not tampered with after it was uploaded
to JSR. This publish attestations feature is not yet implemented, but is planned
for a future date.

Additionally, JSR will provide publish attestations for NPM tarball provided by
JSR. This is also not yet implemented, but is planned for a future date.
