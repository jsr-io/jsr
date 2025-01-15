---
description: This guide explains how to write documentation for JSR packages.
---

Writing documentation is vital for the success of a package. JSR makes it very
easy for package authors to have great documentation, because it generates
documentation based on the JSDoc-like comments in the package source code.

The supported JSDoc-style features and tags are shared with Deno, and are listed
out on
[Deno's Supported JSDoc tags](https://docs.deno.com/runtime/reference/cli/doc/#supported-jsdoc-tags)
page.

This generated documentation is displayed on the package page. This
documentation will also be shown to users in their editor in form of completions
and hover descriptions.

There are two important pieces to documentation:

- **symbol documentation**: this is the documentation for each individual
  function, interface, constant, or class that a package exports
- **module documentation**: this is the documentation for each exported module
  of the package - it acts as an overview or summary for all symbols in the
  module

> Also see this blog post on
> [how to write docs for your package](https://deno.com/blog/document-javascript-package).

## Symbol documentation

To add documentation for a symbol, add a JSDoc comment above the symbol:

```diff
+ /** This function adds the two passed numbers together. */
  export function add(a: number, b: number): number {
    return a + b;
  }
```

The comment can also be a multi-line comment:

```diff
+ /**
+  * This function takes two numbers as input, and then adds these numbers using
+  * floating point math.
+  */
  export function add(a: number, b: number): number {
    return a + b;
  }
```

For functions, documentation can be added to specific parameters or the return
type:

```diff
  /**
   * Search the database with the given query.
   *
+  * @param query This is the query to search with. It should be less than 50 chars to ensure good performance.
+  * @param limit The number of items to return. If unspecified, defaults to 20.
+  * @returns The array of matched items.
   */
  export function search(query: string, limit: number = 20): string[];
```

For more complex symbols, it is often good to include an example demonstrating
how to use the function:

````diff
  /**
   * Search the database with the given query.
   *
+  * ```ts
+  * search("Alan") // ["Alan Turing", "Alan Kay", ...]
+  * ```
   */
  export function search(query: string, limit: number = 20): string[];
````

Interfaces can also be annotated with JSDoc. Their properties and methods can
also be annotated:

```ts
/** The options bag to pass to the {@link search} method. */
export interface SearchOptions {
  /** The maximum number of items to return from the search. Defaults to 50 if
   * unspecified. */
  limit?: number;
  /** Skip the given number of items. This is helpful to implement pagination.
   * Defaults to 0 (do not skip) if not specified. */
  skip?: number;

  /** The function to call if the {@link search} function needs to show warnings
   * to the user. If not specified, warnings will be silently swallowed. */
  reportWarning?(message: string): void;
}
```

> As seen above, `{@link <ident>}` can be used to link between symbols in JSDoc.
> These will become clickable links in the generated documentation.

Classes can be similarly annotated to interfaces and functions:

```ts
/**
 * A class to represent a person.
 */
export class Person {
  /** The name of the person. */
  name: string;
  /** The age of the person. */
  age: number;

  /**
   * Create a new person with the given name and age.
   * @param name The name of the person.
   * @param age The age of the person. Must be non-negative.
   */
  constructor(name: string, age: number) {
    if (age < 0) {
      throw new Error("Age cannot be negative");
    }
    this.name = name;
    this.age = age;
  }

  /** Print a greeting to the console. */
  greet() {
    console.log(
      `Hello, my name is ${this.name} and I am ${this.age} years old.`,
    );
  }
}
```

## Module documentation

Not just symbols can be documented. Modules can also be documented. This is
useful to give an overview of the module and its exported symbols.

To document a module, add a JSDoc comment at the top of the module file, and
include the `@module` tag after the description:

```diff
+ /**
+  * This module contains functions to search the database.
+  * @module
+  */
  
  /** The options bag to pass to the {@link search} method. */
  export interface SearchOptions {}
  
  /** Search the database with the given query. */
  export function search(query: string, options?: SearchOptions): string[];
```

You can also include examples in module documentation:

````diff
  /**
   * This module contains functions to search the database.
   *
+  * @example
+  * ```ts
+  * import { search } from "@luca/search";
+  *
+  * search("Alan") // ["Alan Turing", "Alan Kay", ...]
+  * ```
   *
   * @module
   */
````

If a default entrypoint has a module documentation, it takes precedence over the
README file in the "Overview" tab of the package page.
[Learn more in the documentation section for packages](/docs/packages#documentation).
