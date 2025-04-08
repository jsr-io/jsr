---
title: Using JSR with Cloudflare Workers
description: Learn how to use JSR packages with Cloudflare Workers.
---

[**Cloudflare Workers**](https://workers.cloudflare.com) provides a serverless
execution environment for JavaScript and WebAssembly that can execute your code
on Cloudflare's global network.

JSR packages can be used in Cloudflare Workers by using
[JSR's npm compatibility layer](/docs/npm-compatibility).

First, create a new Cloudflare Worker project:

```shell
npm create cloudflare@latest
yarn create cloudflare
pnpm create cloudflare
bun create cloudflare
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

export default {
  async fetch(request: Request): Promise<Response> {
    return new Response(encodeBase64("Hello World!"));
  },
};
```

Running `wrangler dev` to start the local development server will run your
Cloudflare Worker. You can then visit `http://localhost:8787` to see the result.

```shell
wrangler dev
```

[Learn more about using packages.](/docs/using-packages)
