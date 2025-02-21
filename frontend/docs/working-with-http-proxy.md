---
description: This guide explains how to use JSR behind an HTTP(s) proxy.
---

In some case you may need to use JSR behind an HTTP(s) proxy. This guide will
help you to configure your environment to use JSR behind a proxy.

## With `npm`, `yarn` or `pnpm`

Theses tools understand the `.npmrc` file, which is used to configure the proxy
settings.

> So we recommand to read the official documentation of npm Available
> [here](https://docs.npmjs.com/cli/v11/using-npm/config#https-proxy)

Example of `.npmrc` file:

```ini
proxy=http://proxy.example.com:8080
```

## With `deno`

It's use `HTTP_PROXY` and `HTTPS_PROXY` environment variables. For more
infromation we invite you to read the
[deno docs](https://docs.deno.com/runtime/reference/env_variables/#special-environment-variables)

```sh
export HTTP_PROXY=http://proxy.example.com:8080
export HTTPS_PROXY=http://proxy.example.com:8080
```

and then

```
deno install jsr:@luca/flag
```

## With `bun`

Sadly, bun doesn't support proxy settings yet. _Or it's not obvious how to do
it._
