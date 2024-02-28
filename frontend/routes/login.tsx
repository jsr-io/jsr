// Copyright 2024 the JSR authors. All rights reserved. MIT license.
import { RouteConfig } from "$fresh/server.ts";
import { DevelopmentLogin } from "../islands/DevelopmentLogin.tsx";

export default function Login() {
  return (
    <form class="m-4 space-y-4" method="POST">
      <h1 class="text-2xl font-bold">Development Login for JSR.io</h1>
      <p>
        You can sign in to the local development environment using a production
        token. To do this, click the button below, then approve the
        authorization on the page that opens.
      </p>
      <DevelopmentLogin />
    </form>
  );
}

export const config: RouteConfig = {
  skipInheritedLayouts: true,
};
