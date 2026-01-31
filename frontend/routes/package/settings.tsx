// Copyright 2024 the JSR authors. All rights reserved. MIT license.
import { HttpError, RouteConfig } from "fresh";
import type { Package, RuntimeCompat } from "../../utils/api_types.ts";
import { path } from "../../utils/api.ts";
import { define } from "../../util.ts";
import { PackageGitHubSettings } from "./(_islands)/PackageGitHubSettings.tsx";
import { packageData } from "../../utils/data.ts";
import { PackageHeader } from "./(_components)/PackageHeader.tsx";
import { PackageNav, Params } from "./(_components)/PackageNav.tsx";
import { PackageDescriptionEditor } from "./(_islands)/PackageDescriptionEditor.tsx";
import { RUNTIME_COMPAT_KEYS } from "../../components/RuntimeCompatIndicator.tsx";
import { scopeIAM } from "../../utils/iam.ts";

export default define.page<typeof handler>(
  function Settings({ data, params }) {
    return (
      <div class="mb-20">
        <PackageHeader
          package={data.package}
          downloads={data.downloads}
        />

        <PackageNav
          currentTab="Settings"
          versionCount={data.package.versionCount}
          dependencyCount={data.package.dependencyCount}
          dependentCount={data.package.dependentCount}
          iam={data.iam}
          params={params as unknown as Params}
          latestVersion={data.package.latestVersion}
        />

        <div class="mt-8 flex flex-col gap-12">
          <DescriptionEditor description={data.package.description} />

          <RuntimeCompatEditor runtimeCompat={data.package.runtimeCompat} />

          <GitHubRepository package={data.package} />

          <SelectReadmeSourceEditor source={data.package.readmeSource} />

          <PrivatePackage isPrivate={data.package.isPrivate} />

          <ArchivePackage isArchived={data.package.isArchived} />

          <DeletePackage hasVersions={data.package.versionCount > 0} />

          {data.iam.isStaff && (
            <div class="flex flex-col items-start gap-4">
              <div>
                <h2 class="text-xl font-sans font-bold">Staff area</h2>
                <p class="text-secondary max-w-3xl">
                  Feature a package on the homepage.
                </p>
              </div>

              <form method="POST">
                <FeaturePackage package={data.package} />
              </form>
            </div>
          )}
        </div>
      </div>
    );
  },
);

function GitHubRepository(props: { package: Package }) {
  return (
    <div class="flex flex-col items-start gap-4">
      <div>
        <h2 class="text-xl font-sans font-bold">GitHub Repository</h2>
        <p class="text-secondary max-w-3xl">
          The GitHub repository is shown publicly on the package page.
          Specifying a GitHub repository also enables securely publishing from
          GitHub Actions using OIDC — no need to specify tokens or secrets.{" "}
          <a
            href={`/@${props.package.scope}/${props.package.name}/publish#from-ci`}
            class="link"
          >
            Set up publishing from GitHub Actions.
          </a>
        </p>
      </div>

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
    <form class="flex flex-col items-start gap-4" method="POST">
      <div>
        <h2 class="text-xl font-sans font-bold" id="description">
          Description
        </h2>
        <p class="text-secondary max-w-3xl">
          The package description is shown on the package page and in search
          results.
        </p>
      </div>

      <PackageDescriptionEditor description={props.description} />
    </form>
  );
}

function SelectReadmeSourceEditor(props: { source: "readme" | "jsdoc" }) {
  return (
    <form
      class="flex flex-col items-start gap-4"
      method="POST"
      autocomplete="off"
    >
      <div>
        <h2 class="text-xl font-sans font-bold" id="description">
          Readme Source
        </h2>
        <p class="text-secondary max-w-3xl">
          The source to use to display the content on the main page.
        </p>
      </div>

      <select
        name="source"
        className="input-container input select w-full max-w-sm block py-2 px-4"
      >
        <option value="readme" selected={props.source === "readme"}>
          Readme
        </option>
        <option value="jsdoc" selected={props.source === "jsdoc"}>
          JSDoc (with Readme fallback)
        </option>
      </select>

      <button
        class="button-primary"
        type="submit"
        name="action"
        value="updateReadmeSource"
      >
        Save changes
      </button>
    </form>
  );
}

function RuntimeCompatEditor(props: { runtimeCompat: RuntimeCompat }) {
  return (
    <form class="flex flex-col items-start gap-4" method="POST">
      <div>
        <h2 class="text-xl font-sans font-bold" id="runtime_compat">
          Runtime Compat
        </h2>
        <p class="text-secondary max-w-3xl">
          Set which runtimes this package is compatible with. This information
          is shown on the package page and in search results.
        </p>
      </div>

      <div class="max-w-6xl grid md:grid-cols-2 lg:grid-cols-3 gap-8">
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
        class="button-primary"
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
    <label class="block text-secondary font-bold" htmlFor={id}>
      {name}
      <select
        class="block w-64 py-1.5 px-2 input-container select text-sm font-normal mt-1"
        name={id}
        value={value === undefined ? "" : value ? "true" : "false"}
      >
        <option value="" selected={value === undefined}>
          Compatibility unknown
        </option>
        <option value="true" selected={value === true}>
          ✅ Supported
        </option>
        <option value="false" selected={value === false}>
          ❌ Not supported
        </option>
      </select>
    </label>
  );
}

function PrivatePackage(props: { isPrivate: boolean }) {
  if (!props.isPrivate) {
    return (
      <form class="flex flex-col items-start gap-4" method="POST">
        <div>
          <h2 class="text-xl font-sans font-bold">Make package private</h2>
          <p class="text-secondary max-w-3xl">
            Making a package private restricts access to scope members only.
            Private packages are not visible in search results or on the scope
            page. Users will need a valid bearer token to access the package
            files.
          </p>
        </div>

        <button
          class="button-danger"
          type="submit"
          name="action"
          value="makePrivate"
        >
          Make private
        </button>
      </form>
    );
  } else {
    return (
      <form class="flex flex-col items-start gap-4" method="POST">
        <div>
          <h2 class="text-xl font-sans font-bold">Make package public</h2>
          <p class="text-secondary max-w-3xl">
            Making a package public allows anyone to view and use the package.
            The package will appear in search results and on the scope page.
          </p>
        </div>

        <button
          class="button-danger"
          type="submit"
          name="action"
          value="makePublic"
        >
          Make public
        </button>
      </form>
    );
  }
}

function ArchivePackage(props: { isArchived: boolean }) {
  if (!props.isArchived) {
    return (
      <form class="flex flex-col items-start gap-4" method="POST">
        <div>
          <h2 class="text-xl font-sans font-bold">Archive package</h2>
          <p class="text-secondary max-w-3xl">
            Archiving a package removes it from search indexing and the scope
            page, making it undiscoverable to users. Additionally, you won't be
            able to publish new versions to this package until you unarchive it.
          </p>
        </div>

        <button
          class="button-danger"
          type="submit"
          name="action"
          value="archivePackage"
        >
          Archive package
        </button>
      </form>
    );
  } else {
    return (
      <form class="flex flex-col items-start gap-4" method="POST">
        <div>
          <h2 class="text-xl font-sans font-bold">Unarchive package</h2>
          <p class="text-secondary max-w-3xl">
            Unarchiving a package restores its availability in search results
            and makes it visible on the scope page again. This also allows you
            to publish new versions to the package.
          </p>
        </div>

        <button
          class="button-danger"
          type="submit"
          name="action"
          value="unarchivePackage"
        >
          Unarchive package
        </button>
      </form>
    );
  }
}

function DeletePackage(props: { hasVersions: boolean }) {
  return (
    <form class="flex flex-col items-start gap-4" method="POST">
      <div>
        <h2 class="text-xl font-sans font-bold">Delete package</h2>
        <p class="text-secondary max-w-3xl">
          A package can only be deleted if it has no published versions. This
          action cannot be undone.
        </p>
      </div>

      <button
        class="button-danger"
        disabled={props.hasVersions}
        type="submit"
        name="action"
        value="deletePackage"
      >
        Delete package
      </button>

      {props.hasVersions && (
        <p class="text-red-600">
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
        class="button-danger"
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
      class="button-primary"
      type="submit"
      name="action"
      value="isFeatured"
    >
      Make featured
    </button>
  );
}

export const handler = define.handlers({
  async GET(ctx) {
    const [user, data] = await Promise.all([
      ctx.state.userPromise,
      packageData(ctx.state, ctx.params.scope, ctx.params.package),
    ]);
    if (user instanceof Response) return user;
    if (!data) throw new HttpError(404, "This package was not found.");

    const { pkg, scopeMember, downloads } = data;

    const iam = scopeIAM(ctx.state, scopeMember, user);

    if (!iam.canAdmin) throw new HttpError(404, "This package was not found.");

    ctx.state.meta = {
      title: `Settings - @${pkg.scope}/${pkg.name} - JSR`,
      description: `@${pkg.scope}/${pkg.name} on JSR${
        pkg.description ? `: ${pkg.description}` : ""
      }`,
    };
    return { data: { package: pkg, downloads, iam } };
  },
  async POST(ctx) {
    const req = ctx.req;
    const {
      scope,
      package: packageName,
    } = ctx.params;
    const { api } = ctx.state;
    const data = await req.formData();

    const action = String(data.get("action"));

    switch (action) {
      case "archivePackage": {
        const repoRes = await api.patch(
          path`/scopes/${scope}/packages/${packageName}`,
          { isArchived: true },
        );
        if (!repoRes.ok) throw repoRes;
        return new Response(null, {
          status: 303,
          headers: { Location: `/@${scope}/${packageName}/settings` },
        });
      }
      case "unarchivePackage": {
        const repoRes = await api.patch(
          path`/scopes/${scope}/packages/${packageName}`,
          { isArchived: false },
        );
        if (!repoRes.ok) throw repoRes;
        return new Response(null, {
          status: 303,
          headers: { Location: `/@${scope}/${packageName}/settings` },
        });
      }
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
      case "updateReadmeSource": {
        const sourceRes = await api.patch(
          path`/scopes/${scope}/packages/${packageName}`,
          { readmeSource: data.get("source") },
        );
        if (!sourceRes.ok) {
          throw sourceRes;
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
      case "makePrivate": {
        const repoRes = await api.patch(
          path`/scopes/${scope}/packages/${packageName}`,
          { isPrivate: true },
        );
        if (!repoRes.ok) throw repoRes;
        return new Response(null, {
          status: 303,
          headers: { Location: `/@${scope}/${packageName}/settings` },
        });
      }
      case "makePublic": {
        const repoRes = await api.patch(
          path`/scopes/${scope}/packages/${packageName}`,
          { isPrivate: false },
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
});

export const config: RouteConfig = {
  routeOverride: "/@:scope/:package/settings",
};
