---
description: JSR provides a badges for each package that can be used to showcase the package version or JSR score for a package.
---

For every package, JSR provides an SVG badge that can be included in READMEs,
documentation, or other places to showcase the package version or JSR score for
a package.

## Version Badge

The version badge is a simple way to show the version of a package. It is
available at the following URL:

```
https://jsr.io/badges/@<scope>/<package>
```

It looks like this:

[![JSR](https://jsr.io/badges/@luca/flag)](https://jsr.io/@luca/flag)

To include it in a markdown document, copy the following code and replace
`<scope>` and `<package>` with the scope and package name of the package.

```markdown
[![](https://jsr.io/badges/@<scope>/<package>)](https://jsr.io/@<scope>/<package>)
```

In a HTML document, use the following code:

```html
<a href="https://jsr.io/@<scope>/<package>">
  <img src="https://jsr.io/badges/@<scope>/<package>" alt="">
</a>
```

## JSR Score Badge

The JSR score badge is a way to show the JSR score of a package. It is available
at the following URL:

```
https://jsr.io/badges/@<scope>/<package>/score
```

It looks like this:

[![JSR Score](https://jsr.io/badges/@luca/flag/score)](https://jsr.io/@luca/flag)

To include it in a markdown document, copy the following code and replace
`<scope>` and `<package>` with the scope and package name of the package.

```markdown
[![](https://jsr.io/badges/@<scope>/<package>/score)](https://jsr.io/@<scope>/<package>)
```

In a HTML document, use the following code:

```html
<a href="https://jsr.io/@<scope>/<package>">
  <img src="https://jsr.io/badges/@<scope>/<package>/score" alt="">
</a>
```
