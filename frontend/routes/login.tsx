// Copyright 2024 the JSR authors. All rights reserved. MIT license.
import { asset } from "fresh/runtime";
import { DevelopmentLogin } from "../islands/DevelopmentLogin.tsx";

const PROD_PROXY = !!Deno.env.get("PROD_PROXY");

export default function Login({ url }: { url: URL }) {
  if (PROD_PROXY) {
    return (
      <div class="m-4 space-y-4">
        <h1 class="text-2xl font-bold">Development Login for JSR.io</h1>
        <p>
          You can sign in to the local development environment using a
          production token. To do this, click the button below, then approve the
          authorization on the page that opens.
        </p>
        <DevelopmentLogin />
      </div>
    );
  }

  const redirect = url.searchParams.get("redirect");
  const search = redirect ? `?redirect=${encodeURIComponent(redirect)}` : "";

  return (
    <div class="flex items-center justify-center min-h-[60vh]">
      <div class="space-y-4 text-center">
        <h1 class="text-2xl font-bold">Sign in to JSR</h1>
        <p class="text-secondary">
          Choose a provider to sign in with.
        </p>
        <div class="flex gap-4 flex-col pt-4">
          <a class="button-primary" href={"/login/github" + search}>
            <img class="size-5" src={asset("/logos/github.svg")} />
            Sign in with GitHub
          </a>
          <a class="button-primary" href={"/login/gitlab" + search}>
            <img class="size-5" src={asset("/logos/gitlab.svg")} />
            Sign in with GitLab
          </a>
        </div>
      </div>
    </div>
  );
}
