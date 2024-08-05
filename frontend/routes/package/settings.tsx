// Copyright 2024 the JSR authors. All rights reserved. MIT license.
import { Handlers, PageProps, RouteConfig } from "$fresh/server.ts";
import type { Package, RuntimeCompat } from "../../utils/api_types.ts";
import { path } from "../../utils/api.ts";
import { State } from "../../util.ts";
import { PackageGitHubSettings } from "./(_islands)/PackageGitHubSettings.tsx";
import { packageData } from "../../utils/data.ts";
import { PackageHeader } from "./(_components)/PackageHeader.tsx";
import { PackageNav, Params } from "./(_components)/PackageNav.tsx";
import { PackageDescriptionEditor } from "./(_islands)/PackageDescriptionEditor.tsx";
import { Head } from "$fresh/runtime.ts";
import { RUNTIME_COMPAT_KEYS } from "../../components/RuntimeCompatIndicator.tsx";
import { scopeIAM } from "../../utils/iam.ts";
import { ScopeIAM } from "../../utils/iam.ts";

interface Data {
  package: Package;
  iam: ScopeIAM;
}

export default function Settings({ data, params }: PageProps<Data, State>) {
  return (
    <div class="mb-20">
      <Head>
        <title>
          Settings - @{params.scope}/{params.package} - JSR
        </title>
        <meta
          name="description"
          content={`@${params.scope}/${params.package} on JSR${
            data.package.description ? `: ${data.package.description}` : ""
          }`}
        />
      </Head>

      <PackageHeader package={data.package} />

      <PackageNav
        currentTab="Settings"
        versionCount={data.package.versionCount}
        iam={data.iam}
        params={params as unknown as Params}
        latestVersion={data.package.latestVersion}
      />

      <DescriptionEditor description={data.package.description} />

      <RuntimeCompatEditor runtimeCompat={data.package.runtimeCompat} />

      <GitHubRepository package={data.package} />

      <DeletePackage hasVersions={data.package.versionCount > 0} />

      {data.iam.isStaff && (
        <div class="border-t pt-8 mt-12">
          <h2 class="text-xl font-sans font-bold">Staff area</h2>

          <p class="mt-2 text-jsr-gray-600 max-w-3xl">
            Feature a package on the homepage.
          </p>

          <form method="POST">
            <FeaturePackage package={data.package} />
          </form>
        </div>
      )}
    </div>
  );
}

function GitHubRepository(props: { package: Package }) {
  return (
    <div class="border-t pt-8 mt-12">
      <h2 class="text-xl font-sans font-bold">GitHub Repository</h2>

      <p class="mt-2 text-jsr-gray-600 max-w-3xl">
        The GitHub repository is shown publicly on the package page.
      </p>

      <p class="mt-2 mb-4 text-jsr-gray-600 max-w-3xl">
        Specifying a GitHub repository also enables securely publishing from
        GitHub Actions using OIDC — no need to specify tokens or secrets.{" "}
        <a
          href={`/@${props.package.scope}/${props.package.name}/publish#from-ci`}
          class="text-jsr-cyan-700 hover:underline"
        >
          Set up publishing from GitHub Actions.
        </a>
      </p>

      <PackageGitHubSettings
        scope={props.package.scope}
        package={props.package.name}
        repo={props.package.githubRepository}
      />
    </div>
  );
}

function DescriptionEditor(props: { description: string }) {
  return (
    <form class="mt-8" method="POST">
      <h2 class="text-xl font-sans font-bold" id="description">Description</h2>

      <p class="mt-2 text-jsr-gray-600 max-w-3xl">
        The package description is shown on the package page and in search
        results.
      </p>

      <div class="mt-4 max-w-3xl flex flex-col gap-4">
        <PackageDescriptionEditor description={props.description} />
      </div>
    </form>
  );
}

function RuntimeCompatEditor(props: { runtimeCompat: RuntimeCompat }) {
  return (
    <form class="border-t pt-8 mt-12" method="POST">
      <h2 class="text-xl font-sans font-bold" id="runtime_compat">
        Runtime Compat
      </h2>

      <p class="mt-2 text-jsr-gray-600 max-w-3xl">
        Set which packages this package is compatible with. This information is
        shown on the package page and in search results.
      </p>

      <div class="mt-4 max-w-6xl grid md:grid-cols-2 lg:grid-cols-3 gap-8">
        {RUNTIME_COMPAT_KEYS.map(([key, name]) => (
          <RuntimeCompatEditorItem
            key={key}
            id={key}
            name={name}
            value={props.runtimeCompat[key]}
          />
        ))}
      </div>

      <button
        class="button-primary mt-8"
        type="submit"
        name="action"
        value="updateRuntimeCompat"
      >
        Save changes
      </button>
    </form>
  );
}

function RuntimeCompatEditorItem({ name, id, value }: {
  name: string;
  id: keyof RuntimeCompat;
  value: boolean | undefined;
}) {
  return (
    <label class="block text-jsr-gray-600 font-bold" htmlFor={id}>
      {name}
      <select
        class="block w-64 py-1.5 px-2 input-container select text-sm font-normal mt-1"
        name={id}
        value={value === undefined ? "" : value ? "true" : "false"}
      >
        <option value="">Compatibility unknown</option>
        <option value="true">✅ Supported</option>
        <option value="false">❌ Not supported</option>
      </select>
    </label>
  );
}

function DeletePackage(props: { hasVersions: boolean }) {
  return (
    <form class="border-t pt-8 mt-12" method="POST">
      <h2 class="text-xl font-sans font-bold">Delete package</h2>

      <p class="mt-2 text-jsr-gray-600 max-w-3xl">
        A package can only be deleted if it has no published versions.
        <br />
        This action cannot be undone.
      </p>

      <button
        class="button-danger mt-4"
        disabled={props.hasVersions}
        type="submit"
        name="action"
        value="deletePackage"
      >
        Delete package
      </button>

      {props.hasVersions && (
        <p class="mt-2 text-red-600">
          This package cannot be deleted because it has published versions. Only
          empty packages can be deleted.
        </p>
      )}
    </form>
  );
}

function FeaturePackage(props: { package: Package }) {
  if (props.package.whenFeatured) {
    return (
      <button
        class="button-danger mt-8"
        type="submit"
        name="action"
        value="isNotFeatured"
      >
        Make NOT featured
      </button>
    );
  }

  return (
    <button
      class="button-primary mt-8"
      type="submit"
      name="action"
      value="isFeatured"
    >
      Make featured
    </button>
  );
}

export const handler: Handlers<Data, State> = {
  async GET(_, ctx) {
    const [user, data] = await Promise.all([
      ctx.state.userPromise,
      packageData(ctx.state, ctx.params.scope, ctx.params.package),
    ]);
    if (user instanceof Response) return user;
    if (!data) return ctx.renderNotFound();

    const { pkg, scopeMember } = data;

    const iam = scopeIAM(ctx.state, scopeMember, user);

    if (!iam.canAdmin) return ctx.renderNotFound();

    return ctx.render({ package: pkg, iam });
  },
  async POST(req, ctx) {
    const {
      scope,
      package: packageName,
    } = ctx.params;
    const { api } = ctx.state;
    const data = await req.formData();

    const action = String(data.get("action"));

    switch (action) {
      case "deletePackage": {
        const deleteRes = await api.delete(
          path`/scopes/${scope}/packages/${packageName}`,
        );
        if (!deleteRes.ok) {
          throw deleteRes;
        }
        return new Response(null, {
          status: 303,
          headers: { Location: `/@${scope}` },
        });
      }
      case "updateDescription": {
        const descriptionRes = await api.patch(
          path`/scopes/${scope}/packages/${packageName}`,
          { description: data.get("description") },
        );
        if (!descriptionRes.ok) {
          throw descriptionRes;
        }
        return new Response(null, {
          status: 303,
          headers: { Location: `/@${scope}/${packageName}/settings` },
        });
      }
      case "updateRepo": {
        const owner = String(data.get("owner"));
        const name = String(data.get("repo"));
        const repoRes = await api.patch(
          path`/scopes/${scope}/packages/${packageName}`,
          { githubRepository: { owner, name } },
        );
        if (!repoRes.ok) throw repoRes;
        return new Response(null, {
          status: 303,
          headers: { Location: `/@${scope}/${packageName}/settings` },
        });
      }
      case "unlinkRepo": {
        const repoRes = await api.patch(
          path`/scopes/${scope}/packages/${packageName}`,
          { githubRepository: null },
        );
        if (!repoRes.ok) throw repoRes;
        return new Response(null, {
          status: 303,
          headers: { Location: `/@${scope}/${packageName}/settings` },
        });
      }
      case "updateRuntimeCompat": {
        const runtimeCompat: RuntimeCompat = {};
        for (const [key] of RUNTIME_COMPAT_KEYS) {
          const value = data.get(key);
          if (value === "true") {
            runtimeCompat[key] = true;
          } else if (value === "false") {
            runtimeCompat[key] = false;
          }
        }
        const repoRes = await api.patch(
          path`/scopes/${scope}/packages/${packageName}`,
          { runtimeCompat },
        );
        if (!repoRes.ok) throw repoRes;
        return new Response(null, {
          status: 303,
          headers: { Location: `/@${scope}/${packageName}/settings` },
        });
      }
      case "isFeatured": {
        const repoRes = await api.patch(
          path`/scopes/${scope}/packages/${packageName}`,
          { isFeatured: true },
        );
        if (!repoRes.ok) throw repoRes;
        return new Response(null, {
          status: 303,
          headers: { Location: `/@${scope}/${packageName}/settings` },
        });
      }
      case "isNotFeatured": {
        const repoRes = await api.patch(
          path`/scopes/${scope}/packages/${packageName}`,
          { isFeatured: false },
        );
        if (!repoRes.ok) throw repoRes;
        return new Response(null, {
          status: 303,
          headers: { Location: `/@${scope}/${packageName}/settings` },
        });
      }
      default: {
        throw new Error("Invalid action " + action);
      }
    }
  },
};

export const config: RouteConfig = {
  routeOverride: "/@:scope/:package/settings",
};
