---
description: JSR can integrate with NPM private registries to enable you to use JSR packages inside companies that require the use of an NPM private registry.
---

Private registries are often used inside of companies to control access to
packages, for example to ensure that only approved packages are used. JSR can
integrate with NPM private registries so that you can use JSR packages inside of
companies that require the use of an NPM private registry.

> Note: JSR does not provide private registry functionality itself at this time.
> This means that it is currently not possible to publish private packages to
> JSR. This is a feature that is being tracked in
> [issue #203](https://github.com/jsr-io/jsr/issues/203).

## Using JSR as an upstream for an NPM private registry

When JSR is used as an upstream for an NPM private registry, the private
registry proxies requests to JSR for packages that are not found in the private
registry to JSR. This allows the private registry to be used as a cache for JSR
packages, and enforce rules about which packages may be used within the company.

To use JSR as an upstream for an NPM private registry, you need to configure the
private registry to proxy requests to JSR. The exact steps to do this depend on
the private registry software that you are using. Below are some examples of how
to configure some popular private registry software to use JSR as an upstream.

### Configuring a JSR upstream with Azure DevOps Artifacts

To use JSR as an upstream for Azure DevOps Artifacts, you need to create a new
upstream source in Azure DevOps Artifacts that points to JSR. You can do this by
following these steps:

1. Go to the "Artifacts" section of your Azure DevOps project.
2. Create a new feed or select an existing feed. When creating a new feed, the
   default options will work fine.
3. Click on the "Feed Settings" button in the top right corner of the feed page.
   It is displayed as a gear icon.
4. Click on the "Upstream sources" tab in the feed settings.
5. Click on the "Add Upstream" button.
6. Select "Public Source" when asked about the type of upstream source.
7. In the "Public Source" dropdown, select "Custom registry".
8. Enter the URL of the JSR registry in the "Custom registry URL" field. The URL
   of the JSR registry is `https://npm.jsr.io`.
9. Select "npm" as the "Package type".
10. Enter "JSR" as the "Upstream source name".
11. Click on "Save" to save the upstream source.
12. Select the "JSR" upstream source in the list of upstream sources and press
    the "Move up" button until it is above the "npmjs" upstream source. This
    ensures that JSR is checked before the public NPM registry.
13. Click on "Save" in the top right corner of the upstream sources page to save
    the changes to the upstream sources.

After you have configured JSR as an upstream for Azure DevOps Artifacts, you can
add packages from JSR to your feed in Azure DevOps Artifacts. When a package is
added to the feed, it is accessible to users of the feed. To do this, follow
these steps:

1. Go to the "Artifacts" section of your Azure DevOps project.
2. Select the feed that you want to add the package to. This feed must have JSR
   configured as an upstream (see above).
3. Click on "Search upstream sources" in the feed page.
4. Select "npm" as the package type.
5. Enter the name of the package that you want to add to the feed, in the form:
   `@jsr/scope__package`. For example, to add `@luca/cases`, enter
   `@jsr/luca__cases`.
6. Click on "Search".
7. Select the package and version you'd like to add to the feed, and select "Add
   to feed" from the dropdown menu (three vertical dots next to the package).

After you have added the package to the feed, it will be available to users of
the feed. Users can install the package using the `npm install` command, and the
package will be fetched from JSR. Users do not have to configure JSR as an
upstream in their `.npmrc` file, as the JSR packages will now be available from
the Azure DevOps Artifacts feed.

### Configuring a JSR upstream with Google Artifact Registry

JSR can not yet be used as an upstream for Google Artifact Registry due to a
differing layout of package tarball URLs between JSR and the layout that is
expected by Google Artifact Registry.

Aligning JSRs package tarball URLs with the NPM upstream is being tracked in
[issue #405](https://github.com/jsr-io/jsr/issues/405).

### Configuring a JSR upstream with JFrog Artifactory

JSR can not yet be used as an upstream for JFrog Artifactory due to a differing
layout of package tarball URLs between JSR and the layout that is expected by
JFrog Artifactory.

Aligning JSRs package tarball URLs with the NPM upstream is being tracked in
[issue #405](https://github.com/jsr-io/jsr/issues/405).
