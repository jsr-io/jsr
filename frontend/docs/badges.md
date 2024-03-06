---
description: JSR provides badges for each package that can be used to showcase its latest version or its score.
---

JSR provides badges for each package that can be used to showcase its latest
version or its score.

## Version Badge

The version badge can be used to showcase the version of a package. It is
available at the following URL:

```
https://jsr.io/badges/@<scope>/<package>
```

Here's how it looks:

[![JSR](https://jsr.io/badges/@luca/flag)](https://jsr.io/@luca/flag)

To include it in a Markdown document, use the following code, replacing
`<scope>` and `<package>` with the actual scope and name of the package.

```markdown
[![JSR](https://jsr.io/badges/@<scope>/<package>)](https://jsr.io/@<scope>/<package>)
```

In HTML documents, the following can be used:

```html
<a href="https://jsr.io/@<scope>/<package>">
  <img src="https://jsr.io/badges/@<scope>/<package>" alt="JSR" />
</a>
```

## JSR Score Badge

The JSR score badge can be used to showcase the JSR score of a package. It is
available at the following URL:

```
https://jsr.io/badges/@<scope>/<package>/score
```

Here's how it looks:

[![JSR Score](https://jsr.io/badges/@luca/flag/score)](https://jsr.io/@luca/flag)

To include it in a Markdown document, use the following code, replacing
`<scope>` and `<package>` with the actual scope and name of the package.

```markdown
[![JSR Score](https://jsr.io/badges/@<scope>/<package>/score)](https://jsr.io/@<scope>/<package>)
```

In HTML documents, the following can be used:

```html
<a href="https://jsr.io/@<scope>/<package>">
  <img src="https://jsr.io/badges/@<scope>/<package>/score" alt="JSR Score" />
</a>
```
