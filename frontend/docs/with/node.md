---
title: Using JSR with Node.js
description: Learn how to use JSR packages with Node.js.
---

[**Node.js**](https://nodejs.org) is a JavaScript runtime built on Chrome's V8
JavaScript engine.

JSR packages can be used in Node.js by using
[JSR's npm compatibility layer](/docs/npm-compatibility).

JSR packages are always ESM-only, so your project must be using
`"type": "module"` in your `package.json` to use JSR packages.

You can then add JSR packages with the `jsr` CLI. This will add the package to
your `package.json` and install it into your `node_modules` folder using your
preferred package manager (npm, yarn, or pnpm).

```shell
npx jsr add @std/fmt
```

> Note: You should check the `.npmrc` file that is created into source control.
> Without this, future calls to `npm install` / `yarn` / `pnpm install` will not
> succeed.

You can then import JSR packages in your code:

```ts
import { red } from "@std/fmt/colors";
console.log(red("Hello, world!"));
```

Running this code will print a red "Hello, world!" text to your terminal.

```shell
node main.js
```

[Learn more about using packages.](/docs/using-packages)
