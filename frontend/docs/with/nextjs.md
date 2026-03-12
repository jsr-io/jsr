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
bun create next-app
```

You can then add JSR packages using your package manager. This will add the
package to your `package.json` and install it into your `node_modules` folder
using your preferred package manager (npm, yarn, or pnpm).

```shell
# pnpm 10.9+ and yarn 4.9+
pnpm add jsr:@std/encoding
yarn add jsr:@std/encoding

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
next dev
```

[Learn more about using packages.](/docs/using-packages)
