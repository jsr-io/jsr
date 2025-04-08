---
title: Using JSR with Vite
description: Learn how to use JSR packages with Vite.
---

[**Vite**](https://vitejs.dev) is a build tool that aims to provide a faster and
simpler development experience for modern web projects.

JSR packages can be used in Vite by using
[JSR's npm compatibility layer](/docs/npm-compatibility).

First, create a new Vite project:

```shell
npm init vite@latest
yarn create vite
pnpm create vite
bun create vite
```

You can then add JSR packages using your package manager. This will add the
package to your `package.json` and install it into your `node_modules` folder
using your preferred package manager (npm, yarn, or pnpm).

```shell
# pnpm 10.9+ and yarn 4.9+
pnpm add jsr:@std/encoding
yarn add @std/encoding:<todo>

# npm, bun, and older versions of yarn or pnpm
npx jsr add @std/encoding
bunx jsr add @std/encoding
yarn dlx jsr add @std/encoding
pnpm dlx jsr add @std/encoding
```

> Note: A `.npmrc` file is created when using the JSR CLI. The `.npmrc` file
> that is created should be checked into source control. Without this, future
> calls to `npm install` / `yarn` / `pnpm install` / `bun install` will not
> succeed.

You can then import JSR packages in your code:

```ts
import { encodeBase64 } from "@std/encoding/base64";

document.querySelector<HTMLDivElement>("#app")!.innerHTML = encodeBase64(
  "Hello World!",
);
```

Running `vite` to start the local development server will serve your application
at `http://localhost:5173`. You can then visit `http://localhost:5173` to see
it.

```shell
vite
```

[Learn more about using packages.](/docs/using-packages)
