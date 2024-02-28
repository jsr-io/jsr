// Copyright 2024 the JSR authors. All rights reserved. MIT license.
import { Handlers, PageProps } from "$fresh/server.ts";
import type { State } from "../../util.ts";
import { Table, TableData, TableRow } from "../../components/Table.tsx";
import { path } from "../../utils/api.ts";
import { FullUser, List } from "../../utils/api_types.ts";
import { AdminNav } from "./(_components)/AdminNav.tsx";
import { URLQuerySearch } from "../../components/URLQuerySearch.tsx";

interface Data {
  users: FullUser[];
  query: string;
  page: number;
  limit: number;
  total: number;
}

export default function Waitlist({ data, url }: PageProps<Data>) {
  return (
    <div class="mb-20">
      <AdminNav currentTab="waitlist" />
      <URLQuerySearch query={data.query} />
      <Table
        class="mt-8"
        columns={[
          { title: "Name", class: "w-auto" },
          { title: "E-Mail", class: "w-0" },
          { title: "Created", class: "w-0" },
          { title: "", class: "w-0", align: "right" },
        ]}
        pagination={{ page: data.page, limit: data.limit, total: data.total }}
        currentUrl={url}
      >
        {data.users.map((user) => <UserWaitlistRow user={user} />)}
      </Table>
    </div>
  );
}

function UserWaitlistRow({ user }: { user: FullUser }) {
  return (
    <TableRow>
      <TableData>
        <a href={`/admin/users/${user.id}`}>{user.name}</a>
      </TableData>
      <TableData>{user.email}</TableData>
      <TableData>
        {new Date(user.createdAt).toISOString().slice(0, 10)}
      </TableData>
      <TableData align="right">
        <form action={`/admin/waitlist`} method="POST">
          <input type="hidden" name="user_id" value={user.id} />
          <button
            name="action"
            value="invite"
            type="submit"
            class="button-primary"
          >
            Invite
          </button>
        </form>
      </TableData>
    </TableRow>
  );
}

export const handler: Handlers<Data, State> = {
  async GET(req, ctx) {
    const reqUrl = new URL(req.url);
    const query = reqUrl.searchParams.get("search") || "";
    const page = +(reqUrl.searchParams.get("page") || 1);
    const limit = +(reqUrl.searchParams.get("limit") || 20);

    const resp = await ctx.state.api.get<List<FullUser>>(
      path`/admin/users/waitlisted`,
      { query, page, limit },
    );
    if (!resp.ok) throw resp; // gracefully handle this

    return ctx.render({
      users: resp.data.items,
      total: resp.data.total,
      query,
      page,
      limit,
    });
  },
  async POST(req, ctx) {
    const form = await req.formData();
    const action = form.get("action");
    const userId = form.get("user_id");
    if (action !== "invite" || typeof userId !== "string") {
      throw new Error("Invalid request");
    }

    const resp = await ctx.state.api.post<FullUser>(
      path`/admin/users/${userId}/waitlist_accept`,
      {},
    );
    if (!resp.ok) throw resp; // gracefully handle this

    return new Response(null, {
      status: 303,
      headers: {
        Location: `/admin/waitlist`,
      },
    });
  },
};
