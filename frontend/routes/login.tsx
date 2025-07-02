// Copyright 2024 the JSR authors. All rights reserved. MIT license.
import { RouteConfig } from "fresh";
import { DevelopmentLogin } from "../islands/DevelopmentLogin.tsx";
import { TbBrandGithub, TbBrandGitlab } from "tb-icons";

export default function Login({ url }) {
  if (Deno.env.get("PROD_PROXY")) {
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

  return (
    <div class="mt-12 gap-8 flex items-center justify-center flex-col">
      <a class="button-primary" href={"/login/github" + url.search}>
        <TbBrandGithub class="size-4" /> <span>log in with GitHub</span>
      </a>
      <a class="button-primary" href={"/login/gitlab" + url.search}>
        <TbBrandGitlab class="size-4" /> <span>log in with GitLab</span>
      </a>
    </div>
  );
}

export const config: RouteConfig = {
  skipInheritedLayouts: false,
};
