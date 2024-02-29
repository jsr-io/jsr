// Copyright 2024 the JSR authors. All rights reserved. MIT license.
import { Handlers, PageProps } from "$fresh/server.ts";
import { ModuleHit } from "../packages.tsx";
import { PaginationData, State } from "../../util.ts";
import type {
  FullScope,
  List,
  Package,
  Scope,
  ScopeInvite,
  ScopeMember,
} from "../../utils/api_types.ts";
import { APIResponse, path } from "../../utils/api.ts";
import { ScopeNav } from "./(_components)/ScopeNav.tsx";
import { ScopeHeader } from "./(_components)/ScopeHeader.tsx";
import { scopeDataWithMember } from "../../utils/data.ts";
import { ScopePendingInvite } from "./(_components)/ScopePendingInvite.tsx";
import { Head } from "$fresh/runtime.ts";
import { ListDisplay } from "../../components/List.tsx";

interface Data extends PaginationData {
  scope: Scope | FullScope;
  scopeMember: ScopeMember | null;
  packages: Package[];
  userInvites: ScopeInvite[] | null;
}

export default function ScopePackagesPage(
  { params, data, url, state }: PageProps<Data, State>,
) {
  const isAdmin = data.scopeMember?.user.id === state.user?.id &&
      data.scopeMember?.isAdmin || state.user?.isStaff || false;

  return (
    <div class="mb-20">
      <Head>
        <title>
          @{params.scope} - JSR
        </title>
        <meta
          name="description"
          content={`@${params.scope} on JSR`}
        />
      </Head>
      <ScopeHeader scope={data.scope} />
      <ScopeNav active="Packages" isAdmin={isAdmin} scope={data.scope.scope} />
      <ScopePendingInvite userInvites={data.userInvites} scope={params.scope} />
      <ListDisplay
        pagination={data}
        currentUrl={url}
      >
        {data.packages.map((entry) => ModuleHit(entry))}
      </ListDisplay>
    </div>
  );
}

export const handler: Handlers<Data, State> = {
  async GET(req, ctx) {
    const url = new URL(req.url);
    const page = +(url.searchParams.get("page") || 1);
    // Default to large enough to display all of @std.
    const limit = +(url.searchParams.get("limit") || 50);

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
    if (data === null) return ctx.renderNotFound();
    if (!packagesResp.ok) {
      if (packagesResp.code === "scopeNotFound") return ctx.renderNotFound();
      throw packagesResp; // graceful handle errors
    }
    if (userInvitesResp && !userInvitesResp.ok) throw userInvitesResp;

    return ctx.render({
      scope: data.scope,
      scopeMember: data.scopeMember,
      packages: packagesResp.data.items,
      userInvites: userInvitesResp?.data ?? null,
      page,
      limit,
      total: packagesResp.data.total,
    });
  },
  async POST(req, ctx) {
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
    return new Response(null, {
      status: 303,
      headers: { location: `/@${scope}` },
    });
  },
};
