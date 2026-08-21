// Copyright 2024 the JSR authors. All rights reserved. MIT license.
import { DevelopmentLogin } from "../islands/DevelopmentLogin.tsx";
import { LoginForm } from "../islands/LoginForm.tsx";

const PROD_PROXY = !!process.env.PROD_PROXY;
// Public by design: the site key is shipped to the browser by the widget.
const TURNSTILE_SITE_KEY = process.env.TURNSTILE_SITE_KEY;

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
        <LoginForm search={search} siteKey={TURNSTILE_SITE_KEY} />
      </div>
    </div>
  );
}
