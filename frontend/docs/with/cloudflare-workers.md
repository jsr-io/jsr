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
