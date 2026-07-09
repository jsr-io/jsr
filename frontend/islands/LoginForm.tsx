// Copyright 2024 the JSR authors. All rights reserved. MIT license.
import { useEffect, useRef } from "preact/hooks";
import { useSignal } from "@preact/signals";
import { asset } from "fresh/runtime";

const TURNSTILE_SCRIPT =
  "https://challenges.cloudflare.com/turnstile/v0/api.js?render=explicit";

interface TurnstileOptions {
  sitekey: string;
  callback: (token: string) => void;
  "expired-callback": () => void;
  "error-callback": () => void;
}

interface Turnstile {
  render(el: HTMLElement, options: TurnstileOptions): string;
  remove(widgetId: string): void;
}

declare global {
  // Defined by the Turnstile script once it loads.
  var turnstile: Turnstile | undefined;
}

/**
 * The sign in form. Both providers submit the same form, distinguished by the
 * `formaction` of the button that was pressed, so that the single Turnstile
 * widget below them covers both.
 *
 * The API accepts the resulting POST only with a valid Turnstile token, so the
 * captcha cannot be bypassed by submitting the form by hand.
 *
 * When no site key is configured the widget is not rendered and no token is
 * sent; the API is then also running without a secret key and skips the check.
 */
export function LoginForm(
  { search, siteKey }: { search: string; siteKey?: string },
) {
  const token = useSignal("");
  const unavailable = useSignal(false);
  const container = useRef<HTMLDivElement>(null);

  useEffect(() => {
    if (!siteKey || !container.current) return;
    const el = container.current;
    let widgetId: string | undefined;

    function render() {
      widgetId = globalThis.turnstile?.render(el, {
        sitekey: siteKey!,
        callback: (value) => {
          token.value = value;
          unavailable.value = false;
        },
        // Tokens expire after a few minutes. Clearing the signal re-disables
        // the buttons rather than letting a stale token be submitted.
        "expired-callback": () => token.value = "",
        "error-callback": () => {
          token.value = "";
          unavailable.value = true;
        },
      });
    }

    if (globalThis.turnstile) {
      render();
    } else {
      const script = document.createElement("script");
      script.src = TURNSTILE_SCRIPT;
      script.async = true;
      script.defer = true;
      script.onload = render;
      script.onerror = () => unavailable.value = true;
      document.head.appendChild(script);
    }

    return () => {
      if (widgetId) globalThis.turnstile?.remove(widgetId);
    };
  }, [siteKey]);

  // With no widget there is no token to wait for, so the buttons start live.
  // Otherwise they stay disabled through SSR and hydration until it resolves.
  const ready = !siteKey || token.value !== "";

  return (
    <form method="POST" class="flex gap-4 flex-col pt-4">
      {siteKey && (
        <input type="hidden" name="cf-turnstile-response" value={token.value} />
      )}
      <button
        type="submit"
        class="button-primary"
        formAction={"/login/github" + search}
        disabled={!ready}
      >
        <img class="size-5" src={asset("/logos/github.svg")} alt="" />
        Sign in with GitHub
      </button>
      <button
        type="submit"
        class="button-primary"
        formAction={"/login/gitlab" + search}
        disabled={!ready}
      >
        <img class="size-5" src={asset("/logos/gitlab.svg")} alt="" />
        Sign in with GitLab
      </button>
      {siteKey && (
        <div ref={container} class="flex justify-center min-h-[65px]" />
      )}
      {unavailable.value && (
        <p class="text-sm text-red-600 dark:text-red-400">
          The captcha could not be loaded. Please disable any content blockers
          and refresh the page.
        </p>
      )}
    </form>
  );
}
