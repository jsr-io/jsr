// Copyright 2024 the JSR authors. All rights reserved. MIT license.
import { Handlers, PageProps } from "$fresh/server.ts";
import { ComponentChildren } from "preact";
import { ScopeHeader } from "../(_components)/ScopeHeader.tsx";
import { ScopeNav } from "../(_components)/ScopeNav.tsx";
import { State } from "../../../util.ts";
import { FullScope, User } from "../../../utils/api_types.ts";
import { scopeDataWithMember } from "../../../utils/data.ts";
import { path } from "../../../utils/api.ts";
import { QuotaCard } from "../../../components/QuotaCard.tsx";
import { Head } from "$fresh/runtime.ts";
import { Check } from "../../../components/icons/Check.tsx";
import { ScopeIAM, scopeIAM } from "../../../utils/iam.ts";

interface Data {
  scope: FullScope;
  iam: ScopeIAM;
}

export default function ScopeSettingsPage(
  { params, data, state }: PageProps<Data, State>,
) {
  return (
    <div class="mb-20">
      <Head>
        <title>
          Settings - @{params.scope} - JSR
        </title>
      </Head>
      <ScopeHeader scope={data.scope} />
      <ScopeNav active="Settings" iam={data.iam} scope={data.scope.scope} />
      <ScopeQuotas scope={data.scope} user={state.user!} />
      <GitHubActionsSecurity scope={data.scope} />
      <RequirePublishingFromCI scope={data.scope} />
      <DeleteScope scope={data.scope} />
    </div>
  );
}

function ScopeQuotas({ scope, user }: { scope: FullScope; user: User }) {
  const requestLimitIncreaseBody = `Hello JSR team,
I would like to request a quota increase for my scope.
My user ID is '${user.id}', and my scope is '${scope.scope}'.

Quota to increase:
Amount to increase by:
Reason: `;

  return (
    <div class="mt-8">
      <h2 class="text-lg sm:text-xl font-semibold">Quotas</h2>
      <div class="flex flex-col gap-8">
        <p class="text-gray-600 max-w-2xl">
          Scopes have certain quotas to help prevent abuse. We are happy to
          increase your quotas as needed — just send us an increase request.
        </p>
        <div class="grid grid-cols-1 md:grid-cols-2 lg:grid-cols-3 gap-4">
          <QuotaCard
            title="Total packages"
            description="The total number of packages in the scope."
            limit={scope.quotas.packageLimit}
            usage={scope.quotas.packageUsage}
          />
          <QuotaCard
            title="New packages per week"
            description="The number of new packages that can be created in the scope over a 7 day rolling window."
            limit={scope.quotas.newPackagePerWeekLimit}
            usage={scope.quotas.newPackagePerWeekUsage}
          />
          <QuotaCard
            title="Publish attempts per week"
            description="The number of versions that can be published across all packages in the scope over a 7 day rolling window."
            limit={scope.quotas.publishAttemptsPerWeekLimit}
            usage={scope.quotas.publishAttemptsPerWeekUsage}
          />
        </div>
        <div>
          <a
            href={`mailto:quotas@jsr.io?subject=${
              encodeURIComponent(
                `Scope quota increase for @${scope.scope}`,
              )
            }&body=${encodeURIComponent(requestLimitIncreaseBody)}`}
            class="button-primary"
          >
            Request scope quota increase
          </a>
        </div>
      </div>
    </div>
  );
}

function GitHubActionsSecurity({ scope }: { scope: FullScope }) {
  return (
    <div class="mb-12 mt-12">
      <h2 class="text-lg sm:text-xl font-semibold">GitHub Actions security</h2>
      <p class="mt-2 text-gray-600 max-w-2xl">
        GitHub Actions can be used to publish packages to JSR without having to
        set up authentication tokens. Publishing is permitted only if the
        workflow runs in the GitHub repository that is linked to the package on
        JSR.
      </p>
      <p class="mt-4 text-gray-600 max-w-2xl">
        Additionally, you can restrict publishing to be permitted only if the
        user that triggered the GitHub Actions workflow is a member of this
        scope on JSR.{" "}
      </p>
      <form
        class="mt-8 grid gap-4 md:grid-cols-2 w-full max-w-4xl"
        method="POST"
      >
        <CardButton
          title="Restrict publishing to members"
          description={
            <>
              The GitHub user that triggers the GitHub Actions workflow must be
              a member of this JSR scope, and the workflow must run in the
              GitHub repository linked to the JSR package.
            </>
          }
          selected={scope.ghActionsVerifyActor}
          type="submit"
          name="action"
          value="enableGhActionsVerifyActor"
        />
        <CardButton
          title="Do not restrict publishing"
          description={
            <>
              Any GitHub user with write access to the GitHub repository can
              trigger a GitHub Actions workflow to publish a new version. The
              workflow must run in the GitHub repository linked to the JSR
              package.
            </>
          }
          selected={!scope.ghActionsVerifyActor}
          type="submit"
          name="action"
          value="disableGhActionsVerifyActor"
        />
      </form>
    </div>
  );
}

function RequirePublishingFromCI({ scope }: { scope: FullScope }) {
  return (
    <div class="mb-12 mt-12">
      <h2 class="text-lg sm:text-xl font-semibold">
        Require Publishing from CI
      </h2>
      <p class="mt-2 text-gray-600 max-w-2xl">
        Requiring publishing from CI ensures that all new versions for packages
        in this scope are published from a GitHub Actions workflow. This
        disables the ability to publish with the{" "}
        <span class="font-mono">jsr publish</span>{" "}
        command from a local development environment.
      </p>

      <p class="mt-4 text-gray-600 max-w-2xl">
        This setting is currently{" "}
        <span class="font-semibold">
          {scope.requirePublishingFromCI ? "enabled" : "disabled"}
        </span>. {scope.requirePublishingFromCI
          ? (
            "All new versions for packages in this scope are required to be published from a GitHub Actions workflow."
          )
          : (
            "New versions can be published from CI, or from a local development environment."
          )}
      </p>
      <form
        class="mt-8 max-w-4xl"
        method="POST"
      >
        <input
          type="hidden"
          name="value"
          value={String(!scope.requirePublishingFromCI)}
        />
        <button
          name="action"
          value="requirePublishingFromCI"
          class={scope.requirePublishingFromCI
            ? "button-danger"
            : "button-primary"}
          type="submit"
        >
          {scope.requirePublishingFromCI ? "Disable" : "Enable"}{" "}
          requiring publishing from CI
        </button>
      </form>
    </div>
  );
}

interface CardButtonProps {
  title: ComponentChildren;
  description: ComponentChildren;
  selected?: boolean;
  name?: string;
  value?: string;
  type?: string;
}

function CardButton(props: CardButtonProps) {
  return (
    <button
      class={`grid text-left rounded-xl p-6 group focus-visible:bg-jsr-yellow-50/30 hover:bg-jsr-yellow-50/30 focus-visible:ring-2 outline-none active:bg-gray-100 ring-2 ${
        props.selected ? "ring-jsr-yellow-400" : "ring-jsr-gray-100/50"
      }`}
      type={props.type}
      name={props.name}
      value={props.value}
    >
      <div class="flex justify-between">
        <p class="text-gray-900 font-semibold leading-none">{props.title}</p>
        <div
          class={`-mt-2 -mr-2 h-6 w-6 rounded-full flex-shrink-0 flex justify-center items-center group-focus-visible:ring-2 ring-jsr-yellow-700 ${
            props.selected
              ? "border-1.5 border-jsr-cyan-950 bg-jsr-cyan-950 text-jsr-yellow"
              : "border"
          }`}
        >
          {props.selected && <Check class="stroke-2 size-9" />}
        </div>
      </div>
      <p class="mt-2 w-5/6 text-gray-600 text-sm">{props.description}</p>
    </button>
  );
}

function DeleteScope({ scope }: { scope: FullScope }) {
  const isEmpty = scope.quotas.packageUsage === 0;
  return (
    <form class="mb-8 mt-8" method="POST">
      <h2 class="text-lg font-semibold">Delete scope</h2>
      <p class="mt-2 text-gray-600 max-w-3xl">
        Deleting the scope will immediately allow other users to claim the scope
        and publish packages to it. This action cannot be undone.
      </p>
      <button
        class="mt-4 button-danger"
        disabled={!isEmpty}
        type="submit"
        name="action"
        value="deleteScope"
      >
        Delete scope
      </button>
      {!isEmpty && (
        <p class="mt-4 text-red-600">
          This scope cannot be deleted because it contains packages. Only empty
          scopes can be deleted.
        </p>
      )}
    </form>
  );
}

export const handler: Handlers<Data, State> = {
  async GET(_req, ctx) {
    const [user, data] = await Promise.all([
      ctx.state.userPromise,
      scopeDataWithMember(ctx.state, ctx.params.scope),
    ]);
    if (user instanceof Response) return user;
    if (data === null) return ctx.renderNotFound();

    const iam = scopeIAM(ctx.state, data?.scopeMember, user);
    if (!iam.canAdmin) return ctx.renderNotFound();

    return ctx.render({
      scope: data.scope as FullScope,
      iam,
    });
  },
  async POST(req, ctx) {
    const scope = ctx.params.scope;
    const form = await req.formData();
    const action = String(form.get("action"));
    let enableGhActionsVerifyActor = false;
    switch (action) {
      case "enableGhActionsVerifyActor":
        enableGhActionsVerifyActor = true;
        // fallthrough
      case "disableGhActionsVerifyActor": {
        const res = await ctx.state.api.patch(
          path`/scopes/${scope}`,
          { ghActionsVerifyActor: enableGhActionsVerifyActor },
        );
        if (!res.ok) {
          if (res.code === "scopeNotFound") return ctx.renderNotFound();
          throw res; // graceful handle errors
        }
        return new Response(null, {
          status: 303,
          headers: { Location: `/@${scope}/~/settings` },
        });
      }
      case "requirePublishingFromCI": {
        const value = form.get("value") === "true";
        const res = await ctx.state.api.patch(
          path`/scopes/${scope}`,
          { requirePublishingFromCI: value },
        );
        if (!res.ok) {
          if (res.code === "scopeNotFound") return ctx.renderNotFound();
          throw res; // graceful handle errors
        }
        return new Response(null, {
          status: 303,
          headers: { Location: `/@${scope}/~/settings` },
        });
      }
      case "deleteScope": {
        const res = await ctx.state.api.delete(path`/scopes/${scope}`);
        if (!res.ok) {
          if (res.code === "scopeNotFound") return ctx.renderNotFound();
          throw res; // graceful handle errors
        }
        return new Response(null, {
          status: 303,
          headers: { Location: `/` },
        });
      }
      default:
        throw new Error("Invalid action " + action);
    }
  },
};
