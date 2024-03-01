// Copyright 2024 the JSR authors. All rights reserved. MIT license.

import { Handlers, PageProps, RouteConfig } from "$fresh/server.ts";
import { State } from "../util.ts";
import { Head } from "$fresh/runtime.ts";
import PollPublishingTask from "../islands/PollPublishingTask.tsx";

interface Data {
  noOfPackages: number;
  packageName: string | null;
  date: string;
}

export default function PublishApprovePage({ data }: PageProps<Data>) {
  return (
    <div class="pb-8 mb-16">
      <Head>
        <title>
          Publishing package(s) - JSR
        </title>
      </Head>
      <h1 class="text-4xl font-bold">Publish in progress...</h1>
      {data.noOfPackages > 1 && (
        <p class="text-lg mt-2">
          You have approved publishing of{" "}
          <b>{data.noOfPackages} packages</b>. Go back to the terminal to
          continue.
        </p>
      )}
      {data.packageName && data.noOfPackages === 1 && (
        <>
          <PollPublishingTask date={data.date} packageName={data.packageName} />

          <p className="text-lg mt-2">
            You have approved publishing of <b>{data.packageName}</b>.
          </p>
        </>
      )}
    </div>
  );
}

export const handler: Handlers<Data, State> = {
  async GET(req, ctx) {
    const url = new URL(req.url);
    const noOfPackages = parseInt(
      url.searchParams.get("noOfPackages") ?? "1",
      10,
    );
    const packageName = url.searchParams.get("packageName");
    const date = url.searchParams.get("date") ?? new Date().toISOString();
    const user = await ctx.state.userPromise;
    if (user instanceof Response) return user;

    if (user === null) {
      return new Response(null, {
        status: 302,
        headers: {
          Location: `/login`,
        },
      });
    }

    return ctx.render(
      { noOfPackages, packageName, date },
      { headers: { "X-Robots-Tag": "noindex" } },
    );
  },
};

export const config: RouteConfig = { routeOverride: "/publish-approve" };
