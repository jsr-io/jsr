// Copyright 2024 the JSR authors. All rights reserved. MIT license.
import { HttpError, RouteConfig } from "fresh";
import { define } from "../../util.ts";
import {
  DocsData,
  packageData,
  packageDataWithDocs,
} from "../../utils/data.ts";
import { PackageHeader } from "./(_components)/PackageHeader.tsx";
import { PackageNav, Params } from "./(_components)/PackageNav.tsx";
import { scopeIAM } from "../../utils/iam.ts";
import DiffVersionSelector from "./(_islands)/DiffVersionSelector.tsx";
import { DocsView } from "./(_components)/Docs.tsx";

export default define.page<typeof handler>(function Diff(
  { data, params, state },
) {
  const iam = scopeIAM(state, data.member);

  return (
    <div class="mb-20">
      <PackageHeader
        package={data.package}
        downloads={data.downloads}
      />

      <PackageNav
        currentTab="Diff"
        versionCount={data.package.versionCount}
        dependencyCount={data.package.dependencyCount}
        dependentCount={data.package.dependentCount}
        iam={iam}
        params={params as unknown as Params}
        latestVersion={data.package.latestVersion}
      />

      <div class="pt-8">
        <DiffVersionSelector
          scope={data.package.scope}
          pkg={data.package.name}
          versions={["1.1.3", "1.1.4", "1.1.5"]}
          oldVersion={params.oldVersion}
          newVersion={params.newVersion}
        />
      </div>

      {params.oldVersion && params.newVersion && (
        <DocsView
          docs={data.docs!}
          params={params as unknown as Params}
          selectedVersion={null}
          user={state.user}
          scope={data.package.scope}
          pkg={data.package.name}
        />
      )}
    </div>
  );
});

export const handler = define.handlers({
  async GET(ctx) {
    let res: DocsData;
    if (ctx.params.oldVersion && ctx.params.newVersion) {
      const docsRes = await packageDataWithDocs(
        ctx.state,
        ctx.params.scope,
        ctx.params.package,
        ctx.params.newVersion,
        { all_symbols: "true", oldVersion: ctx.params.oldVersion },
      );
      if (!docsRes) {
        throw new HttpError(
          404,
          "This package or this package version was not found.",
        );
      }
      if (docsRes instanceof Response) {
        return docsRes;
      }
      res = docsRes as DocsData;
    } else {
      const packageRes = await packageData(
        ctx.state,
        ctx.params.scope,
        ctx.params.package,
      );
      res = {
        ...packageRes,
        kind: "content",
        selectedVersion: null,
        selectedVersionIsLatestUnyanked: false,
        docs: null,
      } as DocsData;
    }

    const {
      pkg,
      scopeMember,
      selectedVersion,
      docs,
      downloads,
    } = res as DocsData;

    ctx.state.meta = {
      title: `Score - @${pkg.scope}/${pkg.name} - JSR`,
      description: `@${pkg.scope}/${pkg.name} on JSR${
        pkg.description ? `: ${pkg.description}` : ""
      }`,
    };
    return {
      data: {
        package: pkg,
        downloads: downloads,
        member: scopeMember,
        docs,
      },
      headers: { "X-Robots-Tag": "noindex" },
    };
  },
});

export const config: RouteConfig = {
  routeOverride: "/@:scope/:package/diff/{:oldVersion}?...{:newVersion}?",
};
