// Copyright 2024 the JSR authors. All rights reserved. MIT license.
import { HttpError } from "fresh";
import { define } from "../../util.ts";
import type { List, Package, ScopeInvite } from "../../utils/api_types.ts";
import { APIResponse, path } from "../../utils/api.ts";
import { ScopeNav } from "./(_components)/ScopeNav.tsx";
import { ScopeHeader } from "./(_components)/ScopeHeader.tsx";
import { scopeDataWithMember } from "../../utils/data.ts";
import { ScopePendingInvite } from "./(_components)/ScopePendingInvite.tsx";
import { ListDisplay } from "../../components/List.tsx";
import { PackageHit } from "../../components/PackageHit.tsx";
import { scopeIAM } from "../../utils/iam.ts";

export default define.page<typeof handler>(function ScopePackagesPage(
  { params, data, url, state },
) {
  const iam = scopeIAM(state, data.scopeMember);

  return (
    <div class="mb-20">
      <link hidden rel="stylesheet" href="/api/ddoc/style.css" />
      <link hidden rel="stylesheet" href="/api/ddoc/comrak.css" />
      <script hidden href="/api/ddoc/script.js" />

      <ScopeHeader scope={data.scope} />
      <ScopeNav active="Packages" iam={iam} scope={data.scope.scope} />
      <ScopePendingInvite userInvites={data.userInvites} scope={params.scope} />
      <ListDisplay
        pagination={data}
        currentUrl={url}
        id="packageList"
      >
        {data.packages.map((entry) => PackageHit(entry))}
      </ListDisplay>
      <div class="ddoc hidden space-y-7 mt-7" id="docSearchResults" />
    </div>
  );
});

export const handler = define.handlers({
  async GET(ctx) {
    const page = +(ctx.url.searchParams.get("page") || 1);
    // Default to large enough to display all of @std.
    const limit = +(ctx.url.searchParams.get("limit") || 50);

    const [data, packagesResp, userInvitesResp] = await Promise.all([
      scopeDataWithMember(ctx.state, ctx.params.scope),
      ctx.state.api.get<List<Package>>(
        path`/scopes/${ctx.params.scope}/packages`,
        { page, limit },
      ),
      ctx.state.api.hasToken()
        ? ctx.state.api.get<ScopeInvite[]>(path`/user/invites`)
        : Promise.resolve(null),
    ]);
    if (data === null) throw new HttpError(404, "The scope was not found.");
    if (!packagesResp.ok) {
      if (packagesResp.code === "scopeNotFound") {
        throw new HttpError(404, "The scope was not found.");
      }
      throw packagesResp; // graceful handle errors
    }
    if (userInvitesResp && !userInvitesResp.ok) throw userInvitesResp;

    return {
      data: {
        scope: data.scope,
        scopeMember: data.scopeMember,
        packages: packagesResp.data.items,
        userInvites: userInvitesResp?.data ?? null,
        page,
        limit,
        total: packagesResp.data.total,
      },
    };
  },
  async POST(ctx) {
    const req = ctx.req;
    const scope = ctx.params.scope;
    const form = await req.formData();
    const action = form.get("action");
    let res: APIResponse<null>;
    if (action === "join") {
      res = await ctx.state.api.post<null>(path`/user/invites/${scope}`, null);
    } else if (action === "reject") {
      res = await ctx.state.api.delete<null>(path`/user/invites/${scope}`);
    } else {
      throw new Error("invalid action");
    }
    if (!res.ok) throw res; // graceful handle errors
    ctx.state.meta = {
      title: `@${scope} - JSR`,
      description: `@${scope} on JSR`,
    };
    return ctx.redirect(`/@${scope}`, 303);
  },
});
