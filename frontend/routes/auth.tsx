// Copyright 2024 the JSR authors. All rights reserved. MIT license.

import { HttpError, RouteConfig } from "fresh";
import { ComponentChild } from "preact";
import { define } from "../util.ts";
import Authorize from "../islands/Authorize.tsx";
import { path } from "../utils/api.ts";
import type {
  Authorization,
  Permission,
  PermissionPackagePublishVersion,
} from "../utils/api_types.ts";
import TbChevronRight from "tb-icons/TbChevronRight";

export default define.page<typeof handler>(function AuthPage({ data }) {
  if (data.code === "" || data.authorization === null) {
    return (
      <div>
        <h1 class="text-lg font-semibold">Authorization</h1>
        <p class="mt-2 text-secondary max-w-3xl">
          To authorize a request, enter the code shown in the application.
        </p>
        <form action="/auth" method="GET" class="mt-8">
          <input
            type="text"
            name="code"
            placeholder="ABCD-EFGH"
            class="block w-40 py-1.5 px-3 text-2xl input-container input"
            required
          />
          <button
            type="submit"
            class="button-primary mt-4"
          >
            Continue
          </button>
        </form>
      </div>
    );
  }

  const { publishPermissions } = data;
  const packageNames = publishPermissions.map((perm) =>
    `@${perm.scope}/${perm.package}@${perm.version}`
  );
  return (
    <div class="pb-8 mb-16">
      <h1 class="text-4xl font-bold">Authorization</h1>
      <p class="text-lg mt-2">
        An application is requesting access to your account. It is requesting
        the following permissions:
      </p>
      <div class="mt-8 gap-2">
        {data.authorization.permissions === null && (
          <PermissionTile permission={null} />
        )}
        <PublishPackageList permissions={publishPermissions} />
        {data.authorization.permissions?.filter((perm) =>
          perm.permission !== "package/publish" && !("version" in perm)
        ).map((perm, idx) => <PermissionTile key={idx} permission={perm} />)}
      </div>
      <p class="mt-8">Only grant authorization to applications you trust.</p>
      <Authorize code={data.code} authorizedVersions={packageNames} />
    </div>
  );
});

function PublishPackageList(
  { permissions }: { permissions: PermissionPackagePublishVersion[] },
) {
  if (permissions.length === 0) return null;

  return (
    <ul class="w-full divide-y border-t border-b">
      {permissions.map((perm) => {
        const name = `@${perm.scope}/${perm.package}`;
        return (
          <li
            key={name}
            class="p-1.5 gap-2"
          >
            Publish <b>{name}</b>
            <span class="inline-block ml-2 text-green-600 bg-green-100 px-1 rounded-md">
              {perm.version}
            </span>
          </li>
        );
      })}
    </ul>
  );
}

function PermissionTile({ permission }: { permission: Permission | null }) {
  let icon: ComponentChild;
  let title: string;
  let description: string;

  switch (permission?.permission ?? null) {
    case null:
      icon = <TbChevronRight class="w-12 h-12 shrink-0" />;
      title = "Full access";
      description =
        "Including creating scopes, publishing any package, adding members, removing members, and more";
      break;
    case "package/publish":
      icon = <TbChevronRight class="w-12 h-12 shrink-0" />;
      if ("package" in permission!) {
        title = `Publish any version of @${permission!.scope}/${
          permission!.package
        }`;
        description =
          `This application will be able to publish new versions of the package @${
            permission!.scope
          }/${permission!.package}`;
      } else {
        title = `Publishing any version in @${permission!.scope}`;
        description =
          `This application will be able to publish new versions of any existing package in the scope @${
            permission!.scope
          }`;
      }
      break;

    default:
      throw new Error("unreachable");
  }

  return (
    <div class="p-2 border-2 border-jsr-cyan-300 rounded-md flex items-center gap-2 col-span-2">
      {icon}
      <div>
        <div class="font-semibold text-lg">{title}</div>
        <div>{description}</div>
      </div>
    </div>
  );
}

export const handler = define.handlers({
  async GET(ctx) {
    const code = ctx.url.searchParams.get("code") ?? "";
    const [user, authorizationResp] = await Promise.all([
      ctx.state.userPromise,
      code !== ""
        ? ctx.state.api.get<Authorization>(
          path`/authorizations/details/${code}`,
        )
        : Promise.resolve(null),
    ]);
    if (user instanceof Response) return user;
    if (authorizationResp && !authorizationResp.ok) {
      if (authorizationResp.code === "authorizationNotFound") {
        throw new HttpError(404, "Authorization not found");
      }
      throw authorizationResp; // gracefully handle this
    }

    const authorization = authorizationResp?.data ?? null;

    if (user === null && authorization !== null) {
      const redirectPath = ctx.url.pathname + ctx.url.search;
      return new Response(null, {
        status: 302,
        headers: {
          Location: `/login?redirect=${encodeURIComponent(redirectPath)}`,
        },
      });
    }

    const publishPermissions = authorization?.permissions?.filter((perm) =>
      perm.permission === "package/publish" && "version" in perm
    ) as PermissionPackagePublishVersion[] ?? [];

    let title = "Authorize";
    if (authorization) {
      if (authorization.permissions === null) {
        title += " full access";
      } else {
        title += authorization.permissions.length >= 1 &&
            authorization.permissions[0].permission === "package/publish" &&
            "version" in authorization.permissions[0]
          ? ` publishing @${authorization.permissions[0].scope}/${
            authorization.permissions[0].package
          }${
            publishPermissions.length > 1
              ? ` and ${publishPermissions.length - 1} more`
              : ""
          }`
          : " access";
      }
    }
    ctx.state.meta = { title };
    return {
      data: {
        code,
        authorization: authorizationResp?.data ?? null,
        publishPermissions,
      },
      headers: { "X-Robots-Tag": "noindex" },
    };
  },
});

export const config: RouteConfig = { routeOverride: "/auth" };
