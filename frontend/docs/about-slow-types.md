---
description: JSR uses TypeScript types to generate documentation and improve Node.js compatibility. "Slow types" can get in the way of this.
---

In many of its features, JSR analyzes source code, and in particular TypeScript
types in the source code. This is done to generate documentation, generate type
declarations for the npm compatibility layer, and to speed up type checking of
Deno projects using packages from JSR.

For these features to work, the TypeScript source must not export any functions,
classes, interfaces, or variables, or type aliases, that are themselves or
reference "slow types". "Slow types" are types that are not explicitly written,
or are so complex that they require extensive inference to be understood.

This inference requires full type checking of the TypeScript source, which is
not feasible to do in JSR because it would require running the TypeScript
compiler. It is not feasible because `tsc` does not produce stable type
information over time and is too slow to run on every package in the registry on
a regular basis
([read more in this issue](https://github.com/jsr-io/jsr/issues/444#issuecomment-2079772908)).
Because of this, these kinds of types are not supported in the public API.

<!-- https://github.com/dprint/dprint-plugin-markdown/issues/98 -->
<!--deno-fmt-ignore-start-->
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

"Slow types" occur when a function, class, const declaration, or let declaration
is exported from a package, and its type is not explicitly written or the type
is more complex that what can [be simply inferred](#simple-inference).

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

Slow types that are _internal_ to a package (i.e. are not exported from a file
that is in the `exports` of the package manifest) are not problematic for JSR
and consumers of your package. So, if you have a slow type that is not exported,
you can keep it as is:

```ts
export add(a: number, b: number): number {
  return addInternal(a, b);
}

// It is ok to not explicitly write the return type of this function, because it
// is not exported (it is internal to the package).
function addInternal(a: number, b: number) {
  return a + b;
}
```

## TypeScript restrictions

This section lists out all of the restrictions that the "no slow types" policy
imposes on TypeScript code:

1. All exported functions, classes, and variables must have explicit types. For
   example, functions should have an explicit return type, and classes should
   have explicit types for their properties, and constants should have explicit
   type annotations.

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

Constants should have explicit type annotations:

```diff
- export const GLOBAL_ID = crypto.randomUUID();
+ export const GLOBAL_ID: string = crypto.randomUUID();
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

All types in exported functions, classes, variables, and types must be
[simply inferred](#simple-inference) or explicit. If an expression is too
complex to be inferred, it's type should be explicitly assigned to an
intermediate type.

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

## Simple inference

In a few cases, JSR can infer a type without you needing to specify it
explicitly. These cases are called "simple inference". Types that can be simply
inferred are not considered "slow types".

In general, simple inference is possible if a symbol is does not reference other
symbols. It is also not possible if the TypeScript compiler would perform a type
widening or narrowing operation on the type (for example arrays containing
different shapes of object literals).

Simple inference is possible in two positions:

1. The return type of an arrow function, if the function body is a single simple
   expression and not a block.
2. The type of a variable (const or let declaration) or property that is
   initialized with a simple expression.

```ts
export const foo = 5; // The type of `foo` is `number`.
```

```ts
export const bar = () => 5; // The type of `bar` is `() => number`.
```

```ts
class MyClass {
  prop = 5; // The type of `prop` is `number`.
}
```

This inference can only be performed for a select few simple expressions. These
are:

1. Number literals.
   ```ts
   5;
   1.5;
   ```
2. String literals.
   ```ts,
   "hello";
   // Template strings are not supported.
   ```
3. Boolean literals.
   ```ts
   true;
   false;
   ```
4. `null` and `undefined`.
   ```ts
   null;
   undefined;
   ```
5. BigInt literals.
   ```ts
   5n;
   ```
6. `as T` assertions
   ```ts
   foo() as MyType; // The type is `MyType`.
   ```
7. `Symbol()` and `Symbol.for()` expressions.
   ```ts
   Symbol("foo");
   Symbol.for("foo");
   ```
8. Regular expressions.
   ```ts
   /foo/;
   ```
9. Array literals with simple expressions as properties (excluding object
   literals).
   ```ts
   [1, 2, 3];
   ```
10. Object literals with simple expressions as properties.
    ```ts
    { foo: 5, bar: "hello" };
    ```
11. Functions or arrow functions that are fully annotated (i.e. have all
    parameters and the return type annotated or simply inferred).
    ```ts
    const x = (a: number, b: number): number => a + b;
    ```
