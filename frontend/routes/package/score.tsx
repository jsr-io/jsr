// Copyright 2024 the JSR authors. All rights reserved. MIT license.
import { ComponentChildren } from "preact";
import { HttpError, RouteConfig } from "fresh";
import type { PackageScore } from "../../utils/api_types.ts";
import { path } from "../../utils/api.ts";
import { define } from "../../util.ts";
import { packageData } from "../../utils/data.ts";
import { PackageHeader } from "./(_components)/PackageHeader.tsx";
import { PackageNav, Params } from "./(_components)/PackageNav.tsx";
import { TbAlertCircle, TbCheck, TbX } from "tb-icons";
import { getScoreBgColorClass } from "../../utils/score_ring_color.ts";
import { scopeIAM } from "../../utils/iam.ts";
import { Logo } from "../../components/Logo.tsx";

export default define.page<typeof handler>(function Score(
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
        currentTab="Score"
        versionCount={data.package.versionCount}
        dependencyCount={data.package.dependencyCount}
        dependentCount={data.package.dependentCount}
        iam={iam}
        params={params as unknown as Params}
        latestVersion={data.package.latestVersion}
      />

      {data.package.score
        ? (
          <ScoreInfo
            scope={data.package.scope}
            name={data.package.name}
            scorePercentage={data.package.score}
            score={data.score}
            canAdmin={iam.canAdmin}
          />
        )
        : (
          <div class="mt-8 text-tertiary text-center">
            No score is available for this package, because it does not have a
            stable release.
          </div>
        )}
    </div>
  );
});

function ScoreInfo(props: {
  scope: string;
  name: string;
  scorePercentage: number;
  score: PackageScore;
  canAdmin: boolean;
}) {
  const { scope, name, scorePercentage, score, canAdmin } = props;

  return (
    <div class="mt-8 grid items-center justify-items-center grid-cols-1 md:grid-cols-3 gap-12">
      <div class="w-full h-full flex flex-col items-center justify-center border-1.5 border-jsr-cyan-100 dark:border-jsr-gray-700 rounded-lg p-8 dark:bg-jsr-gray-900">
        <div class="flex gap-2 items-center mb-4">
          <h2 class="text-2xl font-semibold">
            <Logo size="medium" class="inline mr-2" />
            Score
          </h2>
        </div>
        <div class="mb-6">
          @{scope}/{name}
        </div>
        <div
          class={`score-circle flex w-full max-w-32 items-center justify-center aspect-square rounded-full p-1.5 ${
            getScoreBgColorClass(scorePercentage)
          }`}
          style={`--pct: ${scorePercentage}%`}
        >
          <span class="rounded-full w-full h-full bg-white dark:bg-jsr-gray-950 dark:text-gray-200 flex justify-center items-center text-center text-3xl font-bold">
            {scorePercentage}%
          </span>
        </div>
        <div class="text-tertiary text-sm text-center mt-6">
          The JSR score is a measure of the overall quality of a package, based
          on a number of factors such as documentation and runtime
          compatibility.
        </div>
      </div>

      <ul class="flex flex-col divide-jsr-cyan-100 dark:divide-jsr-gray-700 divide-y-1 md:col-span-2 w-full">
        <ScoreItem
          value={score.hasReadme}
          scoreValue={2}
          title="Has a readme or module doc"
        >
          The package should have a README.md in the root of the repository or a
          {" "}
          <a class="link" href="/docs/writing-docs#module-documentation">
            module doc
          </a>{" "}
          in the main entrypoint of the package.
        </ScoreItem>
        <ScoreItem
          value={score.hasReadmeExamples}
          scoreValue={1}
          title="Has examples in the readme or module doc"
        >
          The README or{" "}
          <a class="link" href="/docs/writing-docs#module-documentation">
            module doc
          </a>{" "}
          of the main entrypoint should have an example of how to use the
          package, in the form of a code block.
        </ScoreItem>
        <ScoreItem
          value={score.allEntrypointsDocs}
          scoreValue={1}
          title="Has module docs in all entrypoints"
        >
          Every entrypoint of the package should have a{" "}
          <a class="link" href="/docs/writing-docs#module-documentation">
            module doc
          </a>{" "}
          summarizing what is defined in that module.
        </ScoreItem>
        <ScoreItem
          value={Math.min(score.percentageDocumentedSymbols / 0.8, 1)}
          scoreValue={5}
          title="Has docs for most symbols"
        >
          At least 80% of the packages' exported symbols should have{" "}
          <a class="link" href="/docs/writing-docs#symbol-documentation">
            symbol documentation
          </a>. Currently{" "}
          {Math.floor(score.percentageDocumentedSymbols * 100)}% of exported
          symbols are documented.
        </ScoreItem>
        <ScoreItem
          value={score.allFastCheck}
          scoreValue={5}
          title="No slow types are used"
        >
          The package should not use{" "}
          <a class="link" href="/docs/about-slow-types">
            slow types
          </a>.
        </ScoreItem>
        <ScoreItem
          value={score.hasDescription}
          scoreValue={1}
          title="Has a description"
        >
          The package should have a description set in {canAdmin
            ? (
              <a class="link" href="settings#description">
                the package settings
              </a>
            )
            : "the package settings"}{" "}
          to help users find this package via search.
        </ScoreItem>
        <ScoreItem
          value={score.atLeastOneRuntimeCompatible}
          scoreValue={1}
          title="At least one runtime is marked as compatible"
        >
          The package should be marked with at least one runtime as{" "}
          <span>"compatible"</span> in {canAdmin
            ? (
              <a class="link" href="settings#runtime_compat">
                the package settings
              </a>
            )
            : "the package settings"}{" "}
          to aid users in understanding where they can use this package.
        </ScoreItem>
        <ScoreItem
          value={score.multipleRuntimesCompatible}
          scoreValue={1}
          title="At least two runtimes are marked as compatible"
        >
          The package should be compatible with more than one runtime, and be
          marked as such in {canAdmin
            ? (
              <a class="link" href="settings#runtime_compat">
                the package settings
              </a>
            )
            : "the package settings"}.
        </ScoreItem>
        <ScoreItem
          value={score.hasProvenance}
          scoreValue={1}
          title="Has provenance"
        >
          The package should be published from a verifiable CI/CD workflow, and
          have a{" "}
          <a class="link" href="/docs/trust">
            public transparency log entry
          </a>.
        </ScoreItem>
      </ul>
    </div>
  );
}

function ScoreItem(
  props: {
    title: string;
    children: ComponentChildren;
    value: boolean | number;
    scoreValue: number;
  },
) {
  let status: "complete" | "partial" | "missing";
  if (typeof props.value === "boolean") {
    status = props.value ? "complete" : "missing";
  } else {
    if (props.value === 1) {
      status = "complete";
    } else if (props.value === 0) {
      status = "missing";
    } else {
      status = "partial";
    }
  }

  return (
    <li class="grid grid-cols-[auto_1fr_auto] gap-x-3 py-3 first:pt-0 items-start">
      {status === "complete"
        ? (
          <>
            <TbCheck class="size-6 stroke-green-500 stroke-2 -mt-px" />
            <span class="sr-only">Complete score</span>
          </>
        )
        : (status === "partial"
          ? (
            <>
              <TbAlertCircle class="size-6 stroke-jsr-yellow-500 stroke-2 -mt-px" />
              <span class="sr-only">Partial score</span>
            </>
          )
          : (
            <>
              <TbX class="size-6 stroke-red-500 stroke-2 -mt-px" />
              <span class="sr-only">Missing score</span>
            </>
          ))}

      <div class="max-w-xl pr-2">
        <h3 class="leading-tight">{props.title}</h3>
        <p class="text-tertiary text-sm leading-tight mt-1">
          {props.children}
        </p>
      </div>

      <div class="text-sm text-jsr-gray-400 dark:text-gray-500 pt-[0.2em]">
        {typeof props.value === "number"
          ? (
            <span>
              {Math.floor(props.scoreValue * props.value)}/{props.scoreValue}
            </span>
          )
          : <span>{props.value ? props.scoreValue : 0}/{props.scoreValue}
          </span>}
      </div>
    </li>
  );
}

export const handler = define.handlers({
  async GET(ctx) {
    const [res, scoreResp] = await Promise.all([
      packageData(ctx.state, ctx.params.scope, ctx.params.package),
      ctx.state.api.get<PackageScore>(
        path`/scopes/${ctx.params.scope}/packages/${ctx.params.package}/score`,
      ),
    ]);
    if (res === null) throw new HttpError(404, "This package was not found.");

    if (res.pkg.versionCount < 1) {
      return new Response("", {
        status: 303,
        headers: { Location: `/@${ctx.params.scope}/${ctx.params.package}` },
      });
    }

    // TODO: handle errors gracefully
    if (!scoreResp.ok) throw scoreResp;

    ctx.state.meta = {
      title: `Score - @${res.pkg.scope}/${res.pkg.name} - JSR`,
      description: `@${res.pkg.scope}/${res.pkg.name} on JSR${
        res.pkg.description ? `: ${res.pkg.description}` : ""
      }`,
    };
    return {
      data: {
        package: res.pkg,
        downloads: res.downloads,
        score: scoreResp.data,
        member: res.scopeMember,
      },
      headers: { "X-Robots-Tag": "noindex" },
    };
  },
});

export const config: RouteConfig = {
  routeOverride: "/@:scope/:package/score",
};
