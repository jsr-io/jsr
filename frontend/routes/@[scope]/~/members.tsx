// Copyright 2024 the JSR authors. All rights reserved. MIT license.
import { HttpError } from "fresh";
import { define } from "../../../util.ts";
import { ScopeHeader } from "../(_components)/ScopeHeader.tsx";
import { ScopeNav } from "../(_components)/ScopeNav.tsx";
import { ScopePendingInvite } from "../(_components)/ScopePendingInvite.tsx";
import { ScopeInviteForm } from "../(_islands)/ScopeInviteForm.tsx";
import { ScopeMemberRole } from "../(_islands)/ScopeMemberRole.tsx";
import { Table, TableData, TableRow } from "../../../components/Table.tsx";
import { CopyButton } from "../../../islands/CopyButton.tsx";
import { path } from "../../../utils/api.ts";
import {
  FullUser,
  ScopeInvite,
  ScopeMember,
} from "../../../utils/api_types.ts";
import { scopeData } from "../../../utils/data.ts";
import { TrashCan } from "../../../components/icons/TrashCan.tsx";
import { scopeIAM } from "../../../utils/iam.ts";
import { ScopeIAM } from "../../../utils/iam.ts";

export default define.page<typeof handler>(function ScopeMembersPage(
  { params, data, state, url },
) {
  const iam = scopeIAM(state, data.scopeMember);

  const hasOneAdmin = data.members.filter((member) =>
    member.isAdmin
  ).length === 1;

  const isLastAdmin = (data.scopeMember?.isAdmin || false) && hasOneAdmin;

  const inviteUrl = url.href;

  return (
    <div class="mb-20">
      <ScopeHeader scope={data.scope} />
      <ScopeNav active="Members" iam={iam} scope={data.scope.scope} />
      <ScopePendingInvite
        userInvites={data.invites.filter((i) =>
          i.targetUser.id === state.user?.id
        )}
        scope={params.scope}
      />
      <Table
        class="mt-8"
        columns={[
          { title: "Name", class: "w-auto" },
          { title: "Role", class: "w-0" },
          ...(iam.canAdmin
            ? [{ title: "", class: "w-0", align: "right" as const }]
            : []),
        ]}
        currentUrl={url}
      >
        {data.members.map((member) => (
          <MemberItem
            member={member}
            isLastAdmin={hasOneAdmin && member.isAdmin}
            iam={iam}
          />
        ))}
        {data.invites.map((invite) => (
          <InviteItem
            invite={invite}
            inviteUrl={inviteUrl}
            iam={iam}
          />
        ))}
      </Table>
      {iam.canAdmin && <MemberInvite scope={data.scope.scope} />}
      {data.scopeMember && (
        <MemberLeave
          userId={data.scopeMember.user.id}
          isAdmin={data.scopeMember.isAdmin}
          isLastAdmin={isLastAdmin}
        />
      )}
    </div>
  );
});

interface MemberItemProps {
  isLastAdmin: boolean;
  member: ScopeMember;
  iam: ScopeIAM;
}

export function MemberItem(props: MemberItemProps) {
  const { member, iam } = props;
  return (
    <TableRow key={member.user.id}>
      <TableData>
        <a
          class="text-jsr-cyan-700 hover:text-jsr-cyan-400 hover:underline"
          href={`/user/${member.user.id}`}
        >
          {member.user.name}
        </a>
      </TableData>
      <TableData>
        {iam.canAdmin
          ? (
            <ScopeMemberRole
              scope={member.scope}
              userId={member.user.id}
              isAdmin={member.isAdmin}
              isLastAdmin={props.isLastAdmin}
            />
          )
          : member.isAdmin
          ? "Admin"
          : "Member"}
      </TableData>
      {iam.canAdmin && (
        <TableData>
          <div class="flex gap-2 justify-end">
            <form method="POST" class="contents">
              <input type="hidden" name="userId" value={member.user.id} />
              <button
                class="hover:underline disabled:text-jsr-gray-300 disabled:cursor-not-allowed hover:text-red-600 motion-safe:transition-colors"
                name="action"
                value="deleteMember"
                disabled={props.isLastAdmin}
                title={props.isLastAdmin
                  ? "This is the last admin in this scope. Promote another member to admin before removing this one."
                  : "Remove user"}
              >
                <TrashCan class="h-5 w-5" />
              </button>
            </form>
          </div>
        </TableData>
      )}
    </TableRow>
  );
}

interface InviteItemProps {
  invite: ScopeInvite;
  inviteUrl: string;
  iam: ScopeIAM;
}

export function InviteItem(props: InviteItemProps) {
  const { invite, iam } = props;
  return (
    <TableRow key={invite.targetUser.id} class="striped">
      <TableData>
        <a
          class="text-jsr-cyan-700 hover:text-jsr-cyan-400 hover:underline"
          href={`/user/${invite.targetUser.id}`}
        >
          {invite.targetUser.name}
        </a>
      </TableData>
      <TableData>
        Invited
      </TableData>
      {iam.canAdmin && (
        <TableData>
          <div class="flex justify-end gap-4">
            <CopyButton text={props.inviteUrl} title="Copy invite URL" />
            <form method="POST" class="contents">
              <input type="hidden" name="userId" value={invite.targetUser.id} />
              <button
                class="hover:underline"
                title="Delete invite"
                name="action"
                value="deleteInvite"
              >
                <TrashCan class="h-4 w-4" />
              </button>
            </form>
          </div>
        </TableData>
      )}
    </TableRow>
  );
}

function MemberInvite({ scope }: { scope: string }) {
  return (
    <div class="max-w-3xl border-t border-jsr-cyan-950/10 pt-8 mt-8">
      <h2 class="text-lg font-semibold">Invite member</h2>
      <p class="mt-2 text-jsr-gray-600">
        Inviting users to this scope grants them access to publish all packages
        in this scope and create new packages. They will not be able to manage
        members unless they are granted admin status.
      </p>
      <ScopeInviteForm scope={scope} />
    </div>
  );
}

function MemberLeave(
  props: { userId: string; isAdmin: boolean; isLastAdmin: boolean },
) {
  return (
    <form
      method="POST"
      class="max-w-3xl border-t border-jsr-cyan-950/10 pt-8 mt-12"
    >
      <h2 class="text-lg font-semibold">Leave scope</h2>
      <p class="mt-2 text-jsr-gray-600">
        Leaving this scope will revoke your access to all packages in this
        scope. You will no longer be able to publish packages to this
        scope{props.isAdmin && " or manage members"}.
      </p>
      <input type="hidden" name="userId" value={props.userId} />
      {props.isLastAdmin && (
        <div class="mt-6 border-1 rounded-md border-red-300 bg-red-50 p-6 text-red-600">
          <span class="font-bold text-xl">Warning</span>
          <p>
            You are the last admin in this scope. You must promote another
            member to admin before leaving.
          </p>
        </div>
      )}
      <button
        class="button-danger mt-6"
        type="submit"
        name="action"
        value="deleteMember"
        disabled={props.isLastAdmin}
      >
        Leave
      </button>
    </form>
  );
}

export const handler = define.handlers({
  async GET(ctx) {
    let [user, data, membersResp, invitesResp] = await Promise.all([
      ctx.state.userPromise,
      scopeData(ctx.state, ctx.params.scope),
      ctx.state.api.get<ScopeMember[]>(
        path`/scopes/${ctx.params.scope}/members`,
      ),
      ctx.state.api.hasToken()
        ? ctx.state.api.get<ScopeInvite[]>(
          path`/scopes/${ctx.params.scope}/invites`,
        )
        : Promise.resolve(null),
    ]);
    if (user instanceof Response) return user;
    if (data === null) throw new HttpError(404, "The scope was not found.");
    if (!membersResp.ok) {
      if (membersResp.code === "scopeNotFound") {
        throw new HttpError(404, "The scope was not found.");
      }
      throw membersResp; // graceful handle errors
    }
    if (invitesResp && !invitesResp.ok) {
      if (
        invitesResp.code === "actorNotScopeMember" ||
        invitesResp.code === "actorNotScopeAdmin"
      ) {
        invitesResp = null;
      } else {
        if (invitesResp.code === "scopeNotFound") {
          throw new HttpError(404, "The scope was not found.");
        }
        throw invitesResp; // graceful handle errors
      }
    }

    const scopeMember = membersResp.data.find((member) =>
      member.user.id === (user as FullUser | null)?.id
    ) ?? null;

    ctx.state.meta = {
      title: `Members - @${ctx.params.scope} - JSR`,
      description: `List of members of the @${ctx.params.scope} scope on JSR.`,
    };
    return {
      data: {
        scope: data.scope,
        scopeMember: scopeMember,
        members: membersResp.data,
        invites: invitesResp?.data ?? [],
      },
    };
  },
  async POST(ctx) {
    const req = ctx.req;
    const scope = ctx.params.scope;
    const form = await req.formData();
    const action = form.get("action");
    if (action === "deleteInvite") {
      const userId = String(form.get("userId"));
      const res = await ctx.state.api.delete<null>(
        path`/scopes/${scope}/invites/${userId}`,
      );
      if (!res.ok) {
        if (res.code === "scopeNotFound") {
          throw new HttpError(404, "The scope was not found.");
        }
        throw res; // graceful handle errors
      }
    } else if (action === "deleteMember") {
      const userId = String(form.get("userId"));
      const res = await ctx.state.api.delete<null>(
        path`/scopes/${scope}/members/${userId}`,
      );
      if (!res.ok) {
        if (res.code === "scopeNotFound") {
          throw new HttpError(404, "The scope was not found.");
        }
        throw res; // graceful handle errors
      }
    } else if (action === "invite") {
      const githubLogin = String(form.get("githubLogin"));
      const res = await ctx.state.api.post<ScopeInvite>(
        path`/scopes/${scope}/members`,
        { githubLogin },
      );
      if (!res.ok) {
        if (res.code === "scopeNotFound") {
          throw new HttpError(404, "The scope was not found.");
        }
        throw res; // graceful handle errors
      }
    } else {
      throw new Error("Invalid action");
    }
    return new Response(null, {
      status: 303,
      headers: { Location: `/@${scope}/~/members` },
    });
  },
});
