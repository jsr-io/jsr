// Copyright 2024 the JSR authors. All rights reserved. MIT license.
import { Handlers, PageProps, RouteConfig } from "$fresh/server.ts";
import type {
  Package,
  PackageScore,
  ScopeMember,
} from "../../utils/api_types.ts";
import { path } from "../../utils/api.ts";
import { State } from "../../util.ts";
import { packageData } from "../../utils/data.ts";
import { PackageHeader } from "./(_components)/PackageHeader.tsx";
import { PackageNav, Params } from "./(_components)/PackageNav.tsx";
import { Head } from "$fresh/runtime.ts";
import { Check } from "../../components/icons/Check.tsx";
import { Cross } from "../../components/icons/Cross.tsx";
import { ErrorIcon } from "../../components/icons/Error.tsx";

interface Data {
  package: Package;
  score: PackageScore;
  member: ScopeMember | null;
}

export const MAX_SCORE = 18;

export default function Score(
  { data, params, state }: PageProps<Data, State>,
) {
  const isStaff = state.user?.isStaff || false;
  const canEdit = data.member?.isAdmin || isStaff;
  const scorePercentage = Math.floor((data.score.total / MAX_SCORE) * 100);
  const scoreColorClass = scorePercentage >= 90
    ? "bg-green-500"
    : scorePercentage >= 60
    ? "bg-yellow-500"
    : "bg-red-500";

  return (
    <div class="mb-20">
      <Head>
        <title>
          Dependents - @{params.scope}/{params.package} - JSR
        </title>
      </Head>

      <PackageHeader package={data.package} />

      <PackageNav
        currentTab="Score"
        versionCount={data.package.versionCount}
        canEdit={canEdit}
        params={params as unknown as Params}
        latestVersion={data.package.latestVersion}
      />

      <div class="mt-8 grid items-center justify-items-center grid-cols-1 md:grid-cols-3 gap-12">
        <div class="w-full h-full flex flex-col items-center justify-center border-1.5 border-jsr-cyan-100 rounded-lg p-8">
          <div class="flex gap-2 items-center mb-4">
            <img src="/logo.svg" class="w-16" />
            <h2 class="text-2xl font-semibold">
              <span class="sr-only">JSR</span> Score
            </h2>
          </div>
          <div class="mb-6">
            @{data.package.scope}/{data.package.name}
          </div>
          <div
            class={`flex w-full max-w-32 items-center justify-center aspect-square rounded-full p-1.5 ${scoreColorClass}`}
            style={`background-image: conic-gradient(transparent, transparent ${scorePercentage}%, white ${scorePercentage}%)`}
          >
            <span class="rounded-full w-full h-full bg-white flex justify-center items-center text-center text-3xl font-bold">
              {scorePercentage}%
            </span>
          </div>
          <div class="text-gray-500 text-sm text-center mt-6">
            The JSR score is a measure of the overall quality of a package,
            based on a number of factors such as documentation and runtime
            compatibility.
          </div>
        </div>

        <ul class="flex flex-col gap-y-5 md:col-span-2 md:mr-auto">
          <ScoreItem
            value={data.score.hasReadme}
            scoreValue={2}
            explanation="Has a readme or module doc"
          />
          <ScoreItem
            value={data.score.hasReadmeExamples}
            scoreValue={1}
            explanation="Has examples in the readme or module doc"
          />
          <ScoreItem
            value={data.score.allEntrypointsDocs}
            scoreValue={1}
            explanation="Has module docs in all entrypoints"
          />
          <ScoreItem
            value={data.score.percentageDocumentedSymbols}
            max={5}
            scoreValue={5}
            explanation="Has docs in all symbols"
          />
          <ScoreItem
            value={data.score.allFastCheck}
            scoreValue={5}
            explanation="All entrypoints are fast-check compatible"
          />
          <ScoreItem
            value={data.score.hasDescription}
            scoreValue={1}
            explanation="Has a description"
          />
          <ScoreItem
            value={data.score.atLeastOneRuntimeCompatible}
            scoreValue={1}
            explanation="At least one runtime is marked as compatible"
          />
          <ScoreItem
            value={data.score.multipleRuntimesCompatible}
            scoreValue={1}
            explanation="At least two runtimes are marked as compatible"
          />
        </ul>
      </div>
    </div>
  );
}

function ScoreItem(
  props:
    & { explanation: string; scoreValue: number }
    & ({ value: boolean } | {
      value: number;
      max: number;
    }),
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
    <li class="grid grid-cols-[auto_1fr_auto] gap-x-3 items-start border-b-1.5 border-jsr-cyan-100 pb-0.5">
      {status === "complete"
        ? <Check class="size-6 stroke-green-500 stroke-2 -mt-px" />
        : (status === "partial"
          ? <ErrorIcon class="size-6 stroke-yellow-500 stroke-2 -mt-px" />
          : <Cross class="size-6 stroke-red-500 stroke-2 -mt-px" />)}

      <p class="leading-tight">{props.explanation}</p>

      <div class="text-sm text-gray-400 pt-[0.2em]">
        {typeof props.value === "number"
          ? <span>{Math.floor(props.max * props.value)}/{props.max}</span>
          : <span>{props.value ? props.scoreValue : 0}/{props.scoreValue}
          </span>}
      </div>
    </li>
  );
}

export const handler: Handlers<Data, State> = {
  async GET(req, ctx) {
    const [res, scoreResp] = await Promise.all([
      packageData(ctx.state, ctx.params.scope, ctx.params.package),
      ctx.state.api.get<PackageScore>(
        path`/scopes/${ctx.params.scope}/packages/${ctx.params.package}/score`,
      ),
    ]);
    if (res === null) return ctx.renderNotFound();

    // TODO: handle errors gracefully
    if (!scoreResp.ok) throw scoreResp;

    return ctx.render({
      package: res.pkg,
      score: scoreResp.data,
      member: res.scopeMember,
    }, { headers: { "X-Robots-Tag": "noindex" } });
  },
};

export const config: RouteConfig = {
  routeOverride: "/@:scope/:package/score",
};
