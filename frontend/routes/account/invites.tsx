// Copyright 2024 the JSR authors. All rights reserved. MIT license.
import { HttpError } from "fresh";
import { define } from "../../util.ts";
import { APIResponse, path } from "../../utils/api.ts";
import { ScopeInvite } from "../../utils/api_types.ts";
import { Table, TableData, TableRow } from "../../components/Table.tsx";
import { AccountLayout } from "../account/(_components)/AccountLayout.tsx";

export default define.page<typeof handler>(function AccountInvitesPage(
  { data, url },
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
            <div class="p-3 text-jsr-gray-500 text-center italic">
              You do not have any pending scope invites.
            </div>
          )}
      </div>
    </AccountLayout>
  );
});

function InviteRow({ invite }: { invite: ScopeInvite }) {
  return (
    <TableRow key={invite.scope}>
      <TableData>
        <a
          class="text-jsr-cyan-700 hover:text-blue-400 hover:underline"
          href={`/@${invite.scope}`}
        >
          {invite.scope}
        </a>
      </TableData>
      <TableData>
        <a
          class="text-jsr-cyan-700 hover:text-blue-400 hover:underline"
          href={`/user/${invite.requestingUser.id}`}
        >
          {invite.requestingUser.name}
        </a>
      </TableData>
      <TableData>
        <div class="space-x-4">
          <form id="reject-form" method="POST">
            <input type="hidden" name="scope" value={invite.scope} />
          </form>
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
              form="reject-form"
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

export const handler = define.handlers({
  async GET(ctx) {
    const [currentUser, invitesRes] = await Promise.all([
      ctx.state.userPromise,
      ctx.state.api.get<ScopeInvite[]>(path`/user/invites`),
    ]);
    if (currentUser instanceof Response) return currentUser;
    if (!currentUser) throw new HttpError(404, "No signed in user found.");

    if (!invitesRes.ok) throw invitesRes; // gracefully handle errors
    ctx.state.meta = { title: "Your invites - JSR" };
    return {
      data: {
        user: currentUser,
        invites: invitesRes.data,
      },
    };
  },
  async POST(ctx) {
    const req = ctx.req;
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
    return ctx.redirect(location, 303);
  },
});
