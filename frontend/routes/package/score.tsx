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

  return (
    <div class="mb-20">
      <Head>
        <title>
          Score - @{params.scope}/{params.package} - JSR
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

      <div class="mt-8 grid items-start justify-items-center grid-cols-[max-content_auto] gap-5">
        <div class="grid grid-cols-[max-content_max-content_max-content] items-center gap-x-3 gap-y-5">
          <ScoreItem
            value={data.score.hasReadme}
            scoreValue={2}
            explanation="has a readme or module doc"
          />
          <ScoreItem
            value={data.score.hasReadmeExamples}
            scoreValue={1}
            explanation="has examples in the readme or module doc"
          />
          <ScoreItem
            value={data.score.allEntrypointsDocs}
            scoreValue={1}
            explanation="has module docs in all entrypoints"
          />
          <ScoreItem
            value={data.score.percentageDocumentedSymbols}
            max={5}
            scoreValue={5}
            explanation="has docs in all symbols"
          />
          <ScoreItem
            value={data.score.allFastCheck}
            scoreValue={5}
            explanation="all entrypoints are fast-check compatible"
          />
          <ScoreItem
            value={data.score.hasDescription}
            scoreValue={1}
            explanation="has a description"
          />
          <ScoreItem
            value={data.score.atLeastOneRuntimeCompatible}
            scoreValue={1}
            explanation="at least one runtime is marked as compatible"
          />
          <ScoreItem
            value={data.score.multipleRuntimesCompatible}
            scoreValue={1}
            explanation="at least two runtimes are marked as compatible"
          />
        </div>

        <div class="bg-jsr-cyan-200 rounded px-10 py-4 space-y-4">
          <span>Total score</span>
          <div class="text-center leading-[3rem]">
            <span class="text-3xl font-bold">{data.score.total}</span>
            <span>/{MAX_SCORE}</span>
          </div>
          <div class="text-center leading-[3rem]">
            <span class="text-3xl font-bold">
              {Math.floor((data.score.total / MAX_SCORE) * 100)}
            </span>
            <span>%</span>
          </div>
        </div>
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
    if (props.value === props.max) {
      status = "complete";
    } else if (props.value === 0) {
      status = "missing";
    } else {
      status = "partial";
    }
  }

  return (
    <>
      {status === "complete"
        ? <Check class="size-6 stroke-green-500 stroke-2" />
        : (status === "partial"
          ? <ErrorIcon class="size-6 stroke-yellow-500 stroke-2" />
          : <Cross class="size-6 stroke-red-500 stroke-2" />)}

      <span>{props.explanation}</span>

      <div class="text-sm ml-3">
        {typeof props.value === "number"
          ? <span>{Math.floor(props.max * props.value)}/{props.max}</span>
          : <span>{props.value ? props.scoreValue : 0}/{props.scoreValue}
          </span>}
      </div>
    </>
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
