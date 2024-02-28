---
description: JSR uses TypeScript types to generate docs and improve node compat. "Slow types" can get in the way of this.
---

In many of its features, JSR analyzes source code, and in particular TypeScript
types in the source code. This is done to generate documentation, generate type
declarations for the npm compatibility layer, and to speed up type checking of
projects using JSR packages in Deno.

For these features to work, the TypeScript source must not export any functions,
classes, interfaces, or variables, or type aliases, that are themselves or
reference "slow types". "Slow types" are types that are not explicitly written,
or are so complex that they require extensive inference to be understood.

This inference is too costly to be performed by JSR for these features, so these
kinds of types are not supported in the public API.

<!--deno-fmt-ignore-start-->
<!--https://github.com/dprint/dprint-plugin-markdown/issues/98-->
> :warning: If JSR discovers "slow types" in a package, certain features will
> either not work or degrade in quality. These are:
>
> - Type checking for consumers of the package will be slower. The slowdown is
>   at least on the order of 1.5-2x for most packages. It may be significantly
>   higher.
> - The package will not be able to generate type declarations for the npm
>   compatibility layer, or "slow types" will be omitted or replaced with `any` in
>   the generated type declarations.
> - The package will not be able to generate documentation for the package, or
>   "slow types" will be omitted or missing details in the generated
>   documentation.
<!--deno-fmt-ignore-end-->

## What are slow types?

"Slow types" occur when a function, variable, or interface is exported from a
package, and its type is not explicitly written, or is so complex that it cannot
be simply inferred.

Some examples of "slow types":

```ts
// This function is problematic because the return type is not explicitly
// written, so it would have to be inferred from the function body.
export function foo() {
  return Math.random().toString();
}
```

```ts
const foo = "foo";
const bar = "bar";
export class MyClass {
  // This property is problematic because the type is not explicitly written, so
  // it would have to be inferred from the initializer.
  prop = foo + " " + bar;
}
```

Slow types that are _internal_ to a package (i.e. are not exported) are not
problematic for JSR and consumers of your package.

## TypeScript restrictions

This section lists out all of the restrictions that the "no slow types" policy
imposes on TypeScript code:

1. All exported functions, classes, variables, and types must have explicit
   types. For example, functions should have an explicit return type, and
   classes should have explicit types for their properties.

1. Module augmentation and global augmentation must not be used. This means that
   packages cannot use `declare global`, `declare module`, or
   `export as namespace` to augment the global scope or other modules.

1. CommonJS features must not be used. This means that packages cannot use
   `export =` or `import foo = require("foo")`.

1. All types in exported functions, classes, variables, and types must be simply
   inferred or explicit. If an expression is too complex to be inferred, it's
   type should be explicitly assigned to an intermediate type.

1. Destructuring in exports is not supported. Instead of destructuring, export
   each symbol individually.

1. Types must not reference private fields of classes.

### Explicit types

All symbols exported from a package must explicitly specify types. For example,
functions should have an explicit return type:

```diff
- export function add(a: number, b: number) {
+ export function add(a: number, b: number): number {
    return a + b;
  }
```

Classes should have explicit types for their properties:

```diff
  export class Person {
-   name;
-   age;
+   name: string;
+   age: number;
    constructor(name: string, age: number) {
      this.name = name;
      this.age = age;
    }
  }
```

### Global augmentation

Module augmentation and global augmentation must not be used. This means that
packages can not use `declare global` to introduce new global variables, or
`declare module` to augment other modules.

Here are some examples of unsupported code:

```ts
declare global {
  const globalVariable: string;
}
```

```ts
declare module "some-module" {
  const someModuleVariable: string;
}
```

### CommonJS features

CommonJS features must not be used. This means that packages cannot use
`export =` or `import foo = require("foo")`.

Use ESM syntax instead:

```diff
- export = 5;
+ export default 5;
```

```diff
- import foo = require("foo");
+ import foo from "foo";
```

### Types must be simply inferred or explicit

All types in exported functions, classes, variables, and types must be simply
inferred or explicit. If an expression is too complex to be inferred, it's type
should be explictly assigned to an intermediate type.

For example, in the following case the type of the default export is too complex
to be inferred, so it must be explicitly declared using an intermediate type:

```diff
  class Class {}

- export default {
-   test: new Class(),
- };
+ const obj: { test: Class } = {
+   test: new Class(),
+ };
+
+ export default obj;
```

Or using an `as` assertion:

```diff
  class Class {}
  
  export default {
    test: new Class(),
- };
+ } as { test: Class };
```

For super class expressions, evaluate the expression and assign it to an
intermediate type:

```diff
  interface ISuperClass {}

  function getSuperClass() {
    return class SuperClass implements ISuperClass {};
  }

- export class MyClass extends getSuperClass() {}
+ const SuperClass: ISuperClass = getSuperClass();
+ export class MyClass extends SuperClass {}
```

<!--TODO: example for unsupported-complex-reference-->

### No destructuring in exports

Destructuring in exports is not supported. Instead of destructuring, export each
symbol individually:

```diff
- export const { foo, bar } = { foo: 5, bar: "world" };
+ const obj = { foo: 5, bar: "world" };
+ export const foo: number = obj.foo;
+ export const bar: string = obj.bar;
```

### Types must not reference private fields of the class

Types must not reference private fields of classes during inference.

In this example, a public field references a private field, which is not
allowed.

```diff
  export class MyClass {
-   prop!: typeof MyClass.prototype.myPrivateMember;
-   private myPrivateMember!: string;
+   prop!: MyPrivateMember;
+   private myPrivateMember!: MyPrivateMember;
  }

+ type MyPrivateMember = string;
```
