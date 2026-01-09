// Copyright 2024 the JSR authors. All rights reserved. MIT license.
import { useCallback, useEffect, useRef } from "preact/hooks";
import { Signal, useComputed, useSignal } from "@preact/signals";
import { IS_BROWSER } from "fresh/runtime";
import { Card } from "../../../../components/Card.tsx";
import { CopyButton } from "../../../../islands/CopyButton.tsx";
import TbChevronLeft from "tb-icons/TbChevronLeft";
import { api, APIResponseError, path } from "../../../../utils/api.ts";
import { CreatedToken, Permission } from "../../../../utils/api_types.ts";
import { ErrorDisplay } from "../../../../components/ErrorDisplay.tsx";

export function CreateToken() {
  const usage = useSignal<"publish" | "api" | null>(null);
  const env = useSignal<
    "development" | "github_actions" | "other_ci_service" | null
  >(null);
  const localMachineAnyway = useSignal(false);
  const willStoreSafely = useSignal(false);
  const willBeSafe = useSignal(false);

  if (usage.value === null) {
    return <ChooseUsage usage={usage} />;
  }

  if (usage.value === "publish" && env.value === null) {
    return <ChoosePublishingEnvironment env={env} />;
  }

  if (
    usage.value === "publish" && env.value === "development" &&
    !localMachineAnyway.value
  ) {
    return <LocalMachineHelp localMachineAnyway={localMachineAnyway} />;
  }

  if (usage.value === "publish" && env.value === "github_actions") {
    return <GitHubActionsHelp />;
  }

  if (
    !willStoreSafely.value &&
    (usage.value === "api" ||
      (usage.value === "publish" && env.value !== "other_ci_service"))
  ) {
    return <LocalDangerWarning willStoreSafely={willStoreSafely} />;
  }

  if (!willBeSafe.value) {
    return <FinalDangerWarning willBeSafe={willBeSafe} />;
  }

  return <CreateTokenForm />;
}

function useRadioGroup<T extends string>(
  name: string,
  submitSignal: Signal<T | null>,
) {
  const disabled = useSignal(true);

  const ref = useRef<HTMLFormElement>(null);
  useEffect(() => {
    if (ref.current) {
      const selected = ref.current.elements.namedItem(name) as RadioNodeList;
      if (selected.value) {
        disabled.value = false;
      }
    }
  });

  const onSubmit = useCallback((e: Event) => {
    e.preventDefault();
    const form = e.currentTarget as HTMLFormElement;
    const selected = form.elements.namedItem(name) as RadioNodeList;
    submitSignal.value = selected.value as T;
  }, []);

  const onInput = useCallback((e: Event) => {
    const form = e.currentTarget as HTMLFormElement;
    const selected = form.elements.namedItem(name) as RadioNodeList;
    disabled.value = selected.value === "";
  }, []);

  return { ref, disabled, onSubmit, onInput };
}

function ChooseUsage({ usage }: { usage: Signal<"publish" | "api" | null> }) {
  const { ref, disabled, onSubmit, onInput } = useRadioGroup("path", usage);

  return (
    <Card variant="cyan" filled class="mt-8 max-w-xl">
      <form ref={ref} onSubmit={onSubmit} onInput={onInput}>
        <p class="text-secondary font-semibold">
          What do you plan to do with your personal access token?
        </p>
        <label class="mt-2 flex items-baseline">
          <input type="radio" name="path" value="publish" class="mr-2" />
          Publish packages
        </label>
        <label class="mt-2 flex items-baseline">
          <input type="radio" name="path" value="api" class="mr-2" />
          Interact with the JSR API
        </label>
        <button type="submit" class="button-primary mt-4" disabled={disabled}>
          Next
        </button>
      </form>
    </Card>
  );
}

function ChoosePublishingEnvironment(
  { env }: {
    env: Signal<"development" | "github_actions" | "other_ci_service" | null>;
  },
) {
  const { ref, disabled, onSubmit, onInput } = useRadioGroup("env", env);

  return (
    <Card variant="cyan" filled class="mt-8 max-w-xl">
      <form ref={ref} onSubmit={onSubmit} onInput={onInput}>
        <p class="text-secondary font-semibold">
          What environment do you want to publish from?
        </p>
        <label class="mt-2 flex items-baseline">
          <input type="radio" name="env" value="development" class="mr-2" />
          A development machine
        </label>
        <label class="mt-2 flex items-baseline">
          <input type="radio" name="env" value="github_actions" class="mr-2" />
          GitHub Actions
        </label>
        <label class="mt-2 flex items-baseline">
          <input
            type="radio"
            name="env"
            value="other_ci_service"
            class="mr-2"
          />
          A different CI service
        </label>
        <button type="submit" class="button-primary mt-4" disabled={disabled}>
          Next
        </button>
      </form>
    </Card>
  );
}

function LocalMachineHelp(
  { localMachineAnyway }: { localMachineAnyway: Signal<boolean> },
) {
  return (
    <Card variant="orange" filled class="mt-8 max-w-xl">
      <p class="text-secondary">
        When publishing from a local machine, JSR can interactively authenticate
        you using the web browser. This is much more secure than using a
        personal access token, and is the recommended way to publish packages.
      </p>
      <p class="text-secondary mt-3">
        Do you still want to create a personal access token?
      </p>
      <div class="flex gap-4 mt-4">
        <a
          class="button-primary"
          href="/docs/publishing-packages#publishing-from-your-local-machine"
        >
          Publish without a token
        </a>
        <button
          type="button"
          class="button-danger"
          onClick={() => localMachineAnyway.value = true}
          disabled={!IS_BROWSER}
        >
          Create token anyway
        </button>
      </div>
    </Card>
  );
}

function GitHubActionsHelp() {
  return (
    <Card variant="orange" filled class="mt-8 max-w-xl">
      <p class="text-secondary">
        When publishing from GitHub Actions, JSR authenticates you using OIDC,
        an authentication mechanism that is built into GitHub Actions. It is
        much more secure than using a personal access token.
      </p>
      <p class="text-secondary mt-3">
        You do not need a personal access token for this flow. You can find
        instructions for publishing from GitHub Actions with OIDC in the
        "Publish" tab of your package page.
      </p>
      <a
        class="button-primary mt-4 inline-block"
        href="/docs/publishing-packages#publishing-from-github-actions"
      >
        Learn more about publishing from GitHub Actions
      </a>
    </Card>
  );
}

function LocalDangerWarning(
  { willStoreSafely }: { willStoreSafely: Signal<boolean> },
) {
  return (
    <Card variant="red" filled class="mt-8 max-w-xl">
      <p class="text-secondary">
        Personal access tokens enable a malicious user to impersonate you and
        perform any action you can on JSR,{" "}
        <b>
          including publishing new versions of your packages
        </b>.
      </p>
      <p class="text-secondary mt-3">
        Do not store tokens in your code, in unencrypted local files, or in a
        .bashrc or a similar file. A malicious program could steal your token
        and use it to perform actions on your behalf.
      </p>
      <button
        type="button"
        class="button-danger mt-4"
        onClick={() => willStoreSafely.value = true}
        disabled={!IS_BROWSER}
      >
        I will store the token safely
      </button>
    </Card>
  );
}

function FinalDangerWarning({ willBeSafe }: { willBeSafe: Signal<boolean> }) {
  return (
    <Card variant="red" filled class="mt-8 max-w-xl">
      <p class="text-secondary">
        Personal access tokens are powerful and can be used to perform any
        action you can on JSR,{" "}
        <b>
          including publishing new versions of your packages
        </b>.
      </p>
      <p class="text-secondary mt-3">
        The JSR team will never ask you for you to create or share a personal
        access token. If you are being asked to do so, email{" "}
        <a href="mailto:help@jsr.io" class="link">help@jsr.io</a>{" "}
        immediately and do not proceed.
      </p>
      <button
        type="button"
        class="button-danger mt-4"
        onClick={() => willBeSafe.value = true}
        disabled={!IS_BROWSER}
      >
        I understand the risks, proceed anyway
      </button>
    </Card>
  );
}

function CreateTokenForm() {
  const description = useSignal<string>("");
  const expiry = useSignal<number>(-1);
  const permission = useSignal<"package" | "scope" | "full" | null>(null);
  const scope = useSignal<string>("");
  const name = useSignal<string>("");

  const submitting = useSignal(false);
  const error = useSignal<APIResponseError | null>(null);

  const disabled = useComputed(() => {
    return submitting.value || !(
      description.value.length > 0 &&
      expiry.value >= 0 &&
      permission.value !== null &&
      (permission.value === "full" || scope.value.length > 0) &&
      (permission.value !== "package" || name.value.length > 0)
    );
  });

  const token = useSignal<string | null>(null);

  const onSubmit = useCallback((e: Event) => {
    e.preventDefault();
    if (disabled.value) {
      return;
    }

    const expires = expiry.value === 0
      ? null
      : new Date(Date.now() + expiry.value * 86400 * 1000);
    let permissions!: Permission[] | null;
    switch (permission.value) {
      case "package":
        permissions = [{
          permission: "package/publish",
          scope: scope.value,
          package: name.value,
        }];
        break;
      case "scope":
        permissions = [{ permission: "package/publish", scope: scope.value }];
        break;
      case "full":
        permissions = null;
        break;
    }

    submitting.value = true;
    api.post<CreatedToken>(path`/user/tokens`, {
      description: description.value,
      expiresAt: expires?.toISOString() ?? null,
      permissions,
    }).then((response) => {
      submitting.value = false;

      if (response.ok) {
        token.value = response.data.secret;
      } else {
        error.value = response;
      }
    });
    // Create the token
  }, []);

  if (token.value !== null) {
    return <TokenDisplay token={token.value} />;
  }

  return (
    <form class="max-w-xl mt-8" onSubmit={onSubmit}>
      <DescriptionInput description={description} />
      <ExpiryInput expiry={expiry} />
      <PermissionsInput selected={permission} scope={scope} name={name} />
      <button type="submit" class="button-primary mt-8" disabled={disabled}>
        Create token
      </button>
      {error.value !== null && (
        <div class="mt-8">
          <ErrorDisplay error={error.value} />
        </div>
      )}
    </form>
  );
}

function DescriptionInput({ description }: { description: Signal<string> }) {
  return (
    <label class="block">
      <span class="text-secondary font-semibold block">Description</span>
      <span class="text-tertiary text-sm block">
        A description helps you remember what this token is for.
      </span>
      <input
        type="text"
        class="mt-1 px-3 py-1.5 input-container input w-88 text-sm"
        placeholder="Publish @luca/flag from GitLab CI"
        required
        value={description}
        onInput={(e) =>
          description.value = (e.target as HTMLInputElement).value}
      />
    </label>
  );
}

function ExpiryInput({ expiry }: { expiry: Signal<number> }) {
  return (
    <label class="block mt-8">
      <span class="text-secondary font-semibold block">Expires in</span>
      <span class="text-tertiary text-sm block">
        Tokens that expire are more secure than tokens that never expire.
      </span>
      <select
        class="mt-1 px-3 py-1.5 input-container select w-60 text-sm"
        required
        value={expiry}
        onInput={(e) =>
          expiry.value = Number((e.target as HTMLSelectElement).value)}
      >
        <option value="" hidden selected></option>
        <option value="1">24 hours</option>
        <option value="7">1 week</option>
        <option value="30">1 month</option>
        <option value="90">3 months</option>
        <option value="365">1 year</option>
        <option value="0">Never</option>
      </select>
    </label>
  );
}

function PermissionsInput(
  { selected, scope, name }: {
    selected: Signal<"package" | "scope" | "full" | null>;
    scope: Signal<string>;
    name: Signal<string>;
  },
) {
  const onInput = useCallback((e: Event) => {
    const input = e.target as HTMLInputElement;
    if (input.name === "permission") {
      selected.value = input.value as "package" | "scope" | "full";
    }
  }, []);

  const nameRef = useRef<HTMLInputElement>(null);

  function onPaste(e: ClipboardEvent) {
    const data = e.clipboardData?.getData("text");
    if (typeof data === "string") {
      // Case: luca/flags
      const parts = data.split("/");
      if (parts.length === 2) {
        e.preventDefault();
        scope.value = parts[0];
        name.value = parts[1];
      }
    }
  }

  return (
    <div class="block mt-8" onInput={onInput}>
      <p class="text-secondary font-semibold">Permissions</p>
      <p class="text-tertiary text-sm max-w-2xl">
        Choose the permissions this token should have. More restrictive
        permissions are more secure.
      </p>
      <div class="text-secondary flex flex-col my-2">
        <div class="border bg-green-50 dark:bg-green-950/20 border-green-200 dark:border-green-800 px-3 pb-1 pt-2 rounded-t-lg flex flex-col sm:flex-row justify-between">
          <div>
            <label class="flex items-baseline">
              <input
                type="radio"
                class="mr-2"
                name="permission"
                value="package"
              />
              <span>Publish new versions of this package:</span>
            </label>
            <div class="flex items-center w-[100%-1.25rem] ml-5 mt-1 mb-2 md:w-88 rounded-md text-primary shadow-xs pl-3 py-[2px] pr-[2px] sm:text-sm sm:leading-6 input-container">
              <span class="block">
                @
              </span>
              <input
                class="py-1.5 pr-1 pl-0.5 grow w-0 input"
                type="text"
                name="scope"
                value={scope}
                placeholder="luca"
                disabled={selected.value !== "package"}
                onInput={(e) =>
                  scope.value = (e.target as HTMLInputElement).value}
                onPaste={onPaste}
                onKeyUp={(e) => {
                  // Focus to next input when the user types a "/"
                  if (e.key === "/") {
                    e.preventDefault();
                    const value = e.currentTarget.value.slice(0, -1);
                    e.currentTarget.value = value;
                    scope.value = value;
                    setTimeout(() => {
                      nameRef.current?.focus();
                    }, 0);
                  }
                }}
              />
              <span class="block text-tertiary">/</span>
              <input
                ref={nameRef}
                class="py-1.5 pr-4 pl-1 grow w-0 input rounded-md"
                type="text"
                name="package"
                value={name}
                placeholder="flag"
                disabled={selected.value !== "package"}
                onInput={(e) =>
                  name.value = (e.target as HTMLInputElement).value}
                onPaste={onPaste}
              />
            </div>
          </div>
          <span class="text-sm ml-5 sm:ml-0 text-green-700 dark:text-green-500 sm:mt-1">
            Recommended for CI jobs
          </span>
        </div>

        <div class="border border-t-0 bg-jsr-gray-50 dark:bg-jsr-gray-800/50 border-jsr-gray-200 dark:border-jsr-gray-700 px-3 py-1">
          <label class="flex items-baseline">
            <input type="radio" class="mr-2" name="permission" value="scope" />
            <span>Publish new versions of any packages in this scope:</span>
          </label>
          <div class="flex items-center w-[100%-1.25rem] ml-5 mt-1 mb-2 md:w-64 rounded-md text-primary shadow-xs pl-3 py-[2px] pr-[2px] sm:text-sm sm:leading-6 input-container">
            <span class="block">
              @
            </span>
            <input
              class="py-1.5 pr-1 pl-0.5 grow w-0 input"
              type="text"
              name="scope"
              value={scope}
              placeholder="luca"
              disabled={selected.value !== "scope"}
              onInput={(e) =>
                scope.value = (e.target as HTMLInputElement).value}
            />
          </div>
        </div>

        <div class="border border-t-0 bg-jsr-gray-50 dark:bg-jsr-gray-800/50 border-jsr-gray-200 dark:border-jsr-gray-700 px-3 py-1 rounded-b-lg flex flex-col sm:flex-row justify-between">
          <label class="flex items-baseline">
            <input type="radio" class="mr-2" name="permission" value="full" />
            <span>Full access</span>
          </label>
          <span class="text-sm ml-5 sm:ml-0 text-red-500 sm:mt-0.5">
            Least secure
          </span>
        </div>
      </div>
    </div>
  );
}

function TokenDisplay({ token }: { token: string }) {
  return (
    <Card variant="blue" filled class="mt-8 max-w-xl">
      <p class="text-secondary">
        Your personal access token has been created. Copy it now, as you will
        not be able to see it again.
      </p>
      <code class="mt-4 p-2 pr-4 bg-blue-100 dark:bg-blue-950/40 rounded-md text-sm w-full relative flex items-center justify-between">
        <div>{token}</div>
        <CopyButton text={token} title="Copy token" />
      </code>
      <a href="/account/tokens" class="link flex gap-2 items-center mt-4">
        <TbChevronLeft /> Back to overview
      </a>
    </Card>
  );
}
