---
title: Using JSR with Next.js
description: Learn how to use JSR packages with Next.js.
---

[**Next.js**](https://nextjs.org/) is a React framework for building full-stack
web applications.

JSR packages can be used in Next.js by using
[JSR's npm compatibility layer](/docs/npm-compatibility).

First, create a new Next.js project:

```shell
npx create-next-app@latest
yarn create next-app
pnpm create next-app
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

```tsx
import { encodeBase64 } from "@std/encoding/base64";

export default function Home() {
  return <main>{encodeBase64("Hello, world!")}</main>;
}
```

Running `next dev` to start the local development server will serve your
application at `http://localhost:3000/`. You can then visit
`http://localhost:3000/` to see it.

```shell
$ next dev
```

[Learn more about using JSR packages](/docs/using-packages).
