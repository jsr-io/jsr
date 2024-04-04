// Copyright 2024 the JSR authors. All rights reserved. MIT license.
import { Handlers, PageProps } from "$fresh";
import { State } from "../../util.ts";
import { APIResponse, path } from "../../utils/api.ts";
import { FullUser, ScopeInvite } from "../../utils/api_types.ts";
import { Table, TableData, TableRow } from "../../components/Table.tsx";
import { AccountLayout } from "../account/(_components)/AccountLayout.tsx";

interface Data {
  user: FullUser;
  invites: ScopeInvite[];
}

export default function AccountInvitesPage(
  { data, url }: PageProps<Data, State>,
) {
  return (
    <AccountLayout user={data.user} active="Invites">
      <div>
        {data.invites.length
          ? (
            <Table
              columns={[
                {
                  title: "Scope",
                  class: "w-auto",
                },
                {
                  title: "Invited by",
                  class: "w-auto",
                },
                {
                  title: "",
                  class: "w-0",
                },
              ]}
              currentUrl={url}
            >
              {data.invites.map((invite) => <InviteRow invite={invite} />)}
            </Table>
          )
          : (
            <div class="p-3 text-gray-500 text-center italic">
              You do not have any pending scope invites.
            </div>
          )}
      </div>
    </AccountLayout>
  );
}

function InviteRow({ invite }: { invite: ScopeInvite }) {
  return (
    <TableRow key={invite.scope}>
      <TableData>
        <a
          class="text-cyan-700 hover:text-blue-400 hover:underline"
          href={`/@${invite.scope}`}
        >
          {invite.scope}
        </a>
      </TableData>
      <TableData>
        <a
          class="text-cyan-700 hover:text-blue-400 hover:underline"
          href={`/user/${invite.requestingUser.id}`}
        >
          {invite.requestingUser.name}
        </a>
      </TableData>
      <TableData>
        <div class="space-x-4">
          <form
            class="flex gap-4"
            action={`/@${invite.scope}`}
            method="POST"
          >
            <input type="hidden" name="scope" value={invite.scope} />
            <button
              type="submit"
              class="button-danger py-1 px-4"
              title={`Reject invite to @${invite.scope}`}
              name="action"
              value="reject"
            >
              Reject
            </button>
            <button
              type="submit"
              class="button-primary py-1 px-4"
              title={`Accept invite to @${invite.scope}`}
              name="action"
              value="join"
            >
              Join
            </button>
          </form>
        </div>
      </TableData>
    </TableRow>
  );
}

export const handler: Handlers<Data, State> = {
  async GET(_, ctx) {
    const [currentUser, invitesRes] = await Promise.all([
      ctx.state.userPromise,
      ctx.state.api.get<ScopeInvite[]>(path`/user/invites`),
    ]);
    if (currentUser instanceof Response) return currentUser;
    if (!currentUser) return ctx.renderNotFound();

    if (!invitesRes.ok) throw invitesRes; // gracefully handle errors

    return ctx.render({
      user: currentUser,
      invites: invitesRes.data,
    });
  },
  async POST(req, ctx) {
    const form = await req.formData();
    const action = form.get("action");
    const scope = String(form.get("scope"));
    let res: APIResponse<null>;
    let location = `/account/invites`;
    if (action === "join") {
      res = await ctx.state.api.post<null>(path`/user/invites/${scope}`, null);
      location = `/@${scope}`;
    } else if (action === "reject") {
      res = await ctx.state.api.delete<null>(path`/user/invites/${scope}`);
    } else {
      throw new Error("invalid action");
    }
    if (!res.ok) throw res; // graceful handle errors
    return new Response(null, {
      status: 303,
      headers: { location },
    });
  },
};
