// Copyright 2024 the JSR authors. All rights reserved. MIT license.

import { Signal, signal } from "@preact/signals";
import { define } from "../../util.ts";
import {
  OverallStatus,
  PackageLink,
  PackagePublishStatus,
  VersionPublishStatus,
} from "./(_islands)/publishing.tsx";

export default define.page<typeof handler>(function PublishApprovePage({
  data,
}) {
  const packages = data.authorizedVersions.map((id) => {
    const separator = id.lastIndexOf("@");

    return {
      name: id.slice(0, separator),
      version: id.slice(separator + 1),
      status: signal({ loading: true } satisfies VersionPublishStatus),
    };
  });

  return (
    <div class="pb-8 mb-16">
      <section>
        <h1 class="text-4xl font-bold">Publishing progress</h1>
        <p class="text-lg mt-4">
          You have approved the publishing of {data.authorizedVersions.length}
          {" "}
          package{data.authorizedVersions.length > 1 ? "s" : ""}.
        </p>
        <OverallStatus packages={packages} />

        <ul class="mt-6">
          {packages.map(({ name, version, status }) => (
            <PackageListItem
              name={name}
              version={version}
              date={data.date}
              status={status}
            />
          ))}
        </ul>
      </section>
    </div>
  );
});

function PackageListItem(props: {
  name: string;
  version: string;
  date: string;
  status: Signal<VersionPublishStatus>;
}) {
  return (
    <li class="py-1 px-4 mt-1 border-jsr-gray-200 border">
      <p class="font-semibold text-xl">
        {props.name}
        <span class="text-foreground-secondary text-base">
          @{props.version}
        </span>

        <PackageLink status={props.status} />
      </p>
      <PackagePublishStatus
        name={props.name}
        version={props.version}
        date={props.date}
        status={props.status}
      />
    </li>
  );
}

export const handler = define.handlers({
  async GET(ctx) {
    const authorizedVersions = ctx.url.searchParams.getAll("v");
    const date = ctx.url.searchParams.get("date");
    if (authorizedVersions.length === 0 || !date) {
      return new Response(null, {
        status: 302,
        headers: {
          Location: `/`,
        },
      });
    }

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

    ctx.state.meta = {
      title: `Publishing package${
        authorizedVersions.length > 1 ? "s" : ""
      } - JSR`,
    };
    return {
      data: { authorizedVersions, date },
      headers: { "X-Robots-Tag": "noindex" },
    };
  },
});
