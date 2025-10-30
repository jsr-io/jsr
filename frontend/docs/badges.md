---
description: JSR provides badges for each package that can be used to showcase its latest version or its JSR score.
---

JSR provides badges for each package that can be used to showcase its latest
version or its JSR score.

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
  <img src="https://jsr.io/badges/@<scope>/<package>" alt="" />
</a>
```

## Total Downloads Badge

The total downloads badge can be used to showcase the total amount of downloads
of a package. It is available at the following URL:

```
https://jsr.io/badges/@<scope>/<package>/total-downloads
```

Here's how it looks:

[![JSR](https://jsr.io/badges/@luca/flag/total-downloads)](https://jsr.io/@luca/flag)

To include it in a Markdown document, use the following code, replacing
`<scope>` and `<package>` with the actual scope and name of the package.

```markdown
[![JSR](https://jsr.io/badges/@<scope>/<package>/total-downloads)](https://jsr.io/@<scope>/<package>)
```

In HTML documents, the following can be used:

```html
<a href="https://jsr.io/@<scope>/<package>">
  <img src="https://jsr.io/badges/@<scope>/<package>/total-downloads" alt="" />
</a>
```

## Weekly Downloads Badge

The total downloads badge can be used to showcase the weekly amount of downloads
of a package. It is available at the following URL:

```
https://jsr.io/badges/@<scope>/<package>/weekly-downloads
```

Here's how it looks:

[![JSR](https://jsr.io/badges/@luca/flag/weekly-downloads)](https://jsr.io/@luca/flag)

To include it in a Markdown document, use the following code, replacing
`<scope>` and `<package>` with the actual scope and name of the package.

```markdown
[![JSR](https://jsr.io/badges/@<scope>/<package>/weekly-downloads)](https://jsr.io/@<scope>/<package>)
```

In HTML documents, the following can be used:

```html
<a href="https://jsr.io/@<scope>/<package>">
  <img src="https://jsr.io/badges/@<scope>/<package>/weekly-downloads" alt="" />
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
  <img src="https://jsr.io/badges/@<scope>/<package>/score" alt="" />
</a>
```

## Scope Badge

The scope badge can be used to showcase the package list of a scope. It is
available at the following URL:

```
https://jsr.io/badges/@<scope>
```

Here's how it looks:

[![JSR Scope](https://jsr.io/badges/@luca)](https://jsr.io/@luca)

To include it in a Markdown document, use the following code, replacing
`<scope>` with the actual scope.

```markdown
[![JSR Scope](https://jsr.io/badges/@<scope>)](https://jsr.io/@<scope>)
```

```html
<a href="https://jsr.io/@<scope>">
  <img src="https://jsr.io/badges/@<scope>" alt="" />
</a>
```

## Custom Badge Styling

These badges can be customized by adding query parameters to the URL, for
example:

```
https://jsr.io/badges/@luca?logoColor=f7fd1e&color=f7fd1e&labelColor=083344
```

Here's how it looks:

[![JSR Scope](https://jsr.io/badges/@luca?logoColor=f7fd1e&color=f7fd1e&labelColor=083344)](https://jsr.io/@luca)

The supported style-related query parameters can be found in the
[Shields.io documentation](https://shields.io/badges/endpoint-badge#:~:text=Query%20Parameters).

> Note: `logoSize`, `logo`, `url`, `message` and `cacheSeconds` are not
> supported and if provided, they will be ignored.
