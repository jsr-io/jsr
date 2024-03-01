// Copyright 2024 the JSR authors. All rights reserved. MIT license.
import { ComponentChildren } from "preact";
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
import { getScoreBgColorClass } from "../../utils/score_ring_color.ts";

interface Data {
  package: Package;
  score: PackageScore;
  member: ScopeMember | null;
}

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
        <meta
          name="description"
          content={`@${params.scope}/${params.package} on JSR${
            data.package.description ? `: ${data.package.description}` : ""
          }`}
        />
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
            class={`flex w-full max-w-32 items-center justify-center aspect-square rounded-full p-1.5 ${
              getScoreBgColorClass(data.package.score!)
            }`}
            style={`background-image: conic-gradient(transparent, transparent ${data.package.score}%, #e7e8e8 ${data.package.score}%)`}
          >
            <span class="rounded-full w-full h-full bg-white flex justify-center items-center text-center text-3xl font-bold">
              {data.package.score!}%
            </span>
          </div>
          <div class="text-gray-500 text-sm text-center mt-6">
            The JSR score is a measure of the overall quality of a package,
            based on a number of factors such as documentation and runtime
            compatibility.
          </div>
        </div>

        <ul class="flex flex-col divide-jsr-cyan-100 divide-y-1 md:col-span-2 w-full">
          <ScoreItem
            value={data.score.hasReadme}
            scoreValue={2}
            title="Has a readme or module doc"
          >
            The package should have a README.md in the root of the repository or
            a{" "}
            <a class="link" href="/docs/writing-docs#module-documentation">
              module doc
            </a>{" "}
            in the main entrypoint of the package.
          </ScoreItem>
          <ScoreItem
            value={data.score.hasReadmeExamples}
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
            value={data.score.allEntrypointsDocs}
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
            value={Math.floor(
              Math.min(data.score.percentageDocumentedSymbols / 0.8, 1) * 5,
            )}
            scoreValue={5}
            title="Has docs for most symbols"
          >
            At least 80% of the packages' symbols should have{" "}
            <a class="link" href="/docs/writing-docs#symbol-documentation">
              symbol documentation
            </a>. Currently{" "}
            {(data.score.percentageDocumentedSymbols * 100).toFixed(0)}% of
            symbols are documented.
          </ScoreItem>
          <ScoreItem
            value={data.score.allFastCheck}
            scoreValue={5}
            title="No slow types are used"
          >
            This package uses no{" "}
            <a class="link" href="/docs/about-slow-types">
              slow types
            </a>.
          </ScoreItem>
          <ScoreItem
            value={data.score.hasDescription}
            scoreValue={1}
            title="Has a description"
          >
            The package has a description set in the package settings to help
            users find this package via search.
          </ScoreItem>
          <ScoreItem
            value={data.score.atLeastOneRuntimeCompatible}
            scoreValue={1}
            title="At least one runtime is marked as compatible"
          >
            This package marks at least one runtime as "compatible" in the
            package settings to aid users in understanding where they can use
            this package.
          </ScoreItem>
          <ScoreItem
            value={data.score.multipleRuntimesCompatible}
            scoreValue={1}
            title="At least two runtimes are marked as compatible"
          >
            This package is compatible with more than one runtime, and is marked
            as such in the package settings.
          </ScoreItem>
        </ul>
      </div>
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
        ? <Check class="size-6 stroke-green-500 stroke-2 -mt-px" />
        : (status === "partial"
          ? <ErrorIcon class="size-6 stroke-yellow-500 stroke-2 -mt-px" />
          : <Cross class="size-6 stroke-red-500 stroke-2 -mt-px" />)}

      <div class="max-w-xl pr-2">
        <h3 class="leading-tight">{props.title}</h3>
        <p class="text-gray-500 text-sm leading-tight mt-1">{props.children}</p>
      </div>

      <div class="text-sm text-gray-400 pt-[0.2em]">
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
