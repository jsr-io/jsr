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
```

You can then add JSR packages with the `jsr` CLI. This will add the package to
your `package.json` and install it into your `node_modules` folder using your
preferred package manager (npm, yarn, or pnpm).

```shell
npx jsr add @std/encoding
```

> Note: You should check the `.npmrc` file that is created into source control.
> Without this, future calls to `npm install` / `yarn` / `pnpm install` will not
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
$ vite
```

[Learn more about using packages.](/docs/using-packages)
