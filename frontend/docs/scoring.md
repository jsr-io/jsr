---
description: A JSR score is computed for each package based on the package quality, and is used to rank packages in search.
---

JSR score is a metric assigned to each package automatically by JSR, based on
certain factors that indicate the quality of the package. The score is used to
rank packages in search results, and to help users get a sense of package
quality at a glance.

The JSR score is a percentage between 0 and 100, and is computed based on
factors from 4 high level categories:

- **Documentation**: The presence of a README file, module documentation, and
  documentation for public functions and types.
  [Learn more about writing documentation](/docs/writing-docs).
- **Best practices**: Packages should not use
  [slow types](/docs/about-slow-types), and should be published with
  [package provenance](/docs/trust).
- **Discoverability**: The package should have a description to help users find
  packages via search.
- **Compatibility**: The package should have at least one runtime marked as
  "compatible" in the "Runtime compatibility" section of the package page.
  Additionally, packages are rewarded for having more than one compatible
  runtime.

Each of these categories has different specific factors that contribute to the
score. Each of these factors is weighted differently. You can find the exact
factors and weights in the "Score" tab of the package page.

Currently you do not need to complete all factors to get a 100% score. The exact
wheights and factors are subject to change as we learn more about what makes a
good package.
